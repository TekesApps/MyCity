//! Diagnostic address identity only. Address coverage is never physical credit.
use crate::native_observer::Identity;
use serde::Serialize;
use std::{
    fs::{self, File},
    io::{BufWriter, Write},
    path::{Path, PathBuf},
    time::Instant,
};
pub(crate) const BUFFER_BYTES: usize = 8192;
pub(crate) const TRANSIENT_BOUND_BYTES: usize = 64 * 1024;
const MAX_RANGES: usize = 8193;
pub(crate) const MAX_BYTES: usize = 2 * 1024 * 1024;
fn err(e: impl std::fmt::Display) -> String {
    e.to_string()
}
fn now() -> Result<u64, String> {
    #[cfg(target_os = "macos")]
    {
        crate::r07_runtime::monotonic_ns()
    }
    #[cfg(not(target_os = "macos"))]
    {
        Err("vm_trace_native_platform_unavailable".into())
    }
}
struct Limited {
    file: BufWriter<File>,
    bytes: usize,
}
impl Write for Limited {
    fn write(&mut self, data: &[u8]) -> std::io::Result<usize> {
        if data.len() > MAX_BYTES.saturating_sub(self.bytes) {
            return Err(std::io::Error::other("vm_trace_byte_capacity"));
        }
        self.file.write_all(data)?;
        self.bytes += data.len();
        Ok(data.len())
    }
    fn flush(&mut self) -> std::io::Result<()> {
        self.file.flush()
    }
}
#[derive(Serialize)]
pub(crate) struct Range {
    pub kind: &'static str,
    pub ordinal: usize,
    pub allocation_generation: u64,
    pub start: usize,
    pub len: usize,
    pub capacity: usize,
    /// Logical byte interval represented by this physical mapping, when applicable.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub logical_start: Option<usize>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub logical_len: Option<usize>,
    pub nested_in_arena: bool,
    pub grant_bytes: Option<usize>,
}
#[derive(Serialize)]
pub(crate) struct Summary {
    pub list_id: u64,
    pub epoch: u64,
    pub ranges: usize,
    pub bytes: usize,
    pub file: PathBuf,
}
pub(crate) struct Writer {
    out: Limited,
    pending: PathBuf,
    path: PathBuf,
    owner: Identity,
    begin: u64,
    list_id: u64,
    epoch: u64,
    expected: usize,
    count: usize,
}
impl Writer {
    pub fn new(path: &Path, list_id: u64, epoch: u64, expected: usize) -> Result<Self, String> {
        if expected > MAX_RANGES {
            return Err("vm_trace_range_capacity".into());
        }
        let begin = now()?;
        let owner = crate::native_observer::capture(std::process::id(), None, Instant::now())
            .map_err(|e| format!("vm_trace_identity:{e:?}"))?
            .identity;
        if path.exists() {
            return Err("vm_trace_existing_list".into());
        }
        let pending = path.with_extension("jsonl.pending");
        let file = File::create_new(&pending).map_err(err)?;
        let mut writer = Self {
            out: Limited {
                file: BufWriter::with_capacity(BUFFER_BYTES, file),
                bytes: 0,
            },
            pending,
            path: path.into(),
            owner,
            begin,
            list_id,
            epoch,
            expected,
            count: 0,
        };
        writer.line(&serde_json::json!({"record":"begin","schema":"mycity-vm-address-list-v1","source_sha256":env!("MYCITY_SOURCE_SHA256"),"owner":owner,"list_id":list_id,"epoch":epoch,"begin_ns":begin,"expected_ranges":expected,"address_coverage_is_not_physical_credit":true,"buffer_capacity_bytes":BUFFER_BYTES,"transient_bound_bytes":TRANSIENT_BOUND_BYTES,"list_byte_cap":MAX_BYTES}))?;
        Ok(writer)
    }
    fn line(&mut self, value: &impl Serialize) -> Result<(), String> {
        serde_json::to_writer(&mut self.out, value).map_err(err)?;
        self.out.write_all(b"\n").map_err(err)
    }
    pub fn range(&mut self, range: Range) -> Result<(), String> {
        if self.count >= self.expected
            || range.start == 0
            || range.capacity == 0
            || range.len > range.capacity
            || range.start.checked_add(range.capacity).is_none()
        {
            return Err("vm_trace_range_shape_or_count".into());
        }
        self.line(&range)?;
        self.count += 1;
        Ok(())
    }
    pub fn arena(&mut self, a: mycity_role_allocator::Activation) -> Result<(), String> {
        self.range(Range {
            kind: "role_arena",
            ordinal: 0,
            allocation_generation: 0,
            start: a.arena_start,
            len: a.arena_bytes,
            capacity: a.arena_bytes,
            logical_start: None,
            logical_len: None,
            nested_in_arena: false,
            grant_bytes: Some(a.grant_bytes),
        })
    }
    pub fn finish(mut self) -> Result<Summary, String> {
        if self.count != self.expected {
            return Err("vm_trace_incomplete_list".into());
        }
        let owner =
            crate::native_observer::capture(std::process::id(), Some(self.owner), Instant::now())
                .map_err(|e| format!("vm_trace_identity:{e:?}"))?
                .identity;
        let end = now()?;
        self.line(&serde_json::json!({"record":"end","complete":true,"owner":owner,"list_id":self.list_id,"epoch":self.epoch,"begin_ns":self.begin,"end_ns":end,"expected_ranges":self.expected,"actual_ranges":self.count}))?;
        self.out.flush().map_err(err)?;
        self.out.file.get_ref().sync_all().map_err(err)?;
        fs::rename(&self.pending, &self.path).map_err(err)?;
        Ok(Summary {
            list_id: self.list_id,
            epoch: self.epoch,
            ranges: self.count,
            bytes: self.out.bytes,
            file: self.path,
        })
    }
}
#[cfg(all(test, target_os = "macos"))]
mod tests {
    use super::*;
    #[test]
    fn missing_end_and_invalid_ranges_never_publish_a_complete_list() {
        let dir = std::env::temp_dir().join(format!(
            "mycity-vmtrace-{}-{}",
            std::process::id(),
            now().unwrap()
        ));
        fs::create_dir(&dir).unwrap();
        let path = dir.join("ranges.jsonl");
        let writer = Writer::new(&path, 1, 0, 1).unwrap();
        assert!(writer.finish().is_err());
        assert!(!path.exists());
        let path2 = dir.join("invalid.jsonl");
        let mut writer = Writer::new(&path2, 2, 0, 1).unwrap();
        assert!(writer
            .range(Range {
                kind: "bad",
                ordinal: 0,
                allocation_generation: 0,
                start: usize::MAX,
                len: 2,
                capacity: 2,
                logical_start: None,
                logical_len: None,
                nested_in_arena: false,
                grant_bytes: None
            })
            .is_err());
        drop(writer);
        assert!(!path2.exists());
        fs::remove_dir_all(dir).unwrap();
    }
}
