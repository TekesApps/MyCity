//! Candidate safe macOS resource reader. No memory credit and no model authority.
//! Every observation binds Mach birth and parent around the actual resource read.
use serde::{Deserialize, Serialize};
use std::{
    fs::{self, File},
    io::{Read, Write},
    path::Path,
    time::Instant,
};

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Identity {
    pub pid: u32,
    pub start_abstime: u64,
    pub parent_pid: u32,
}
#[derive(Clone, Copy, Debug, Serialize, Deserialize)]
pub struct Usage {
    pub start_abstime: u64,
    pub exit_abstime: u64,
    pub resident_bytes: u64,
    pub footprint_bytes: u64,
    pub user_time_raw: u64,
    pub system_time_raw: u64,
    pub disk_read_bytes: u64,
    pub disk_written_bytes: u64,
}
#[derive(Clone, Copy, Debug, Serialize, Deserialize)]
pub struct Sample {
    pub identity: Identity,
    pub begin_ns: u64,
    pub end_ns: u64,
    pub usage: Usage,
}
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum Unknown {
    InvalidPid,
    ResourceUnavailable,
    RelationUnavailable,
    IdentityChanged,
    ExpectedIdentityMismatch,
    Exited,
    UnsupportedPlatform,
}

trait Reader {
    fn usage(&mut self, pid: u32) -> Result<Usage, Unknown>;
    fn parent(&mut self, pid: u32) -> Result<u32, Unknown>;
}
fn checked_capture(
    reader: &mut impl Reader,
    pid: u32,
    expected: Option<Identity>,
    clock: impl Fn() -> u64,
) -> Result<Sample, Unknown> {
    if pid == 0 || i32::try_from(pid).is_err() {
        return Err(Unknown::InvalidPid);
    }
    let begin_ns = clock();
    let first = reader.usage(pid)?;
    let parent_pid = reader.parent(pid)?;
    let second_parent = reader.parent(pid)?;
    let last = reader.usage(pid)?;
    let end_ns = clock();
    if first.exit_abstime != 0 || last.exit_abstime != 0 {
        return Err(Unknown::Exited);
    }
    if first.start_abstime == 0
        || first.start_abstime != last.start_abstime
        || parent_pid != second_parent
    {
        return Err(Unknown::IdentityChanged);
    }
    let identity = Identity {
        pid,
        parent_pid,
        start_abstime: last.start_abstime,
    };
    if expected.is_some_and(|e| e != identity) {
        return Err(Unknown::ExpectedIdentityMismatch);
    }
    Ok(Sample {
        identity,
        begin_ns,
        end_ns,
        usage: last,
    })
}
struct Native;
#[cfg(target_os = "macos")]
impl Reader for Native {
    fn usage(&mut self, pid: u32) -> Result<Usage, Unknown> {
        use libproc::pid_rusage::{pidrusage, RUsageInfoV4};
        let pid = i32::try_from(pid).map_err(|_| Unknown::InvalidPid)?;
        let value = pidrusage::<RUsageInfoV4>(pid).map_err(|_| Unknown::ResourceUnavailable)?;
        Ok(Usage {
            start_abstime: value.ri_proc_start_abstime,
            exit_abstime: value.ri_proc_exit_abstime,
            resident_bytes: value.ri_resident_size,
            footprint_bytes: value.ri_phys_footprint,
            user_time_raw: value.ri_user_time,
            system_time_raw: value.ri_system_time,
            disk_read_bytes: value.ri_diskio_bytesread,
            disk_written_bytes: value.ri_diskio_byteswritten,
        })
    }
    fn parent(&mut self, pid: u32) -> Result<u32, Unknown> {
        use libproc::{bsd_info::BSDInfo, proc_pid::pidinfo};
        let value = pidinfo::<BSDInfo>(i32::try_from(pid).map_err(|_| Unknown::InvalidPid)?, 0)
            .map_err(|_| Unknown::RelationUnavailable)?;
        if value.pbi_pid != pid {
            return Err(Unknown::IdentityChanged);
        }
        Ok(value.pbi_ppid)
    }
}
#[cfg(not(target_os = "macos"))]
impl Reader for Native {
    fn usage(&mut self, _: u32) -> Result<Usage, Unknown> {
        Err(Unknown::UnsupportedPlatform)
    }
    fn parent(&mut self, _: u32) -> Result<u32, Unknown> {
        Err(Unknown::UnsupportedPlatform)
    }
}
pub fn capture(pid: u32, expected: Option<Identity>, origin: Instant) -> Result<Sample, Unknown> {
    checked_capture(&mut Native, pid, expected, || {
        origin.elapsed().as_nanos().min(u128::from(u64::MAX)) as u64
    })
}
#[cfg(target_os = "macos")]
fn abi() -> serde_json::Value {
    use libproc::{bsd_info::BSDInfo, pid_rusage::RUsageInfoV4};
    serde_json::json!({
        "rusage_size": std::mem::size_of::<RUsageInfoV4>(),
        "rusage_offsets": {
            "ri_user_time": std::mem::offset_of!(RUsageInfoV4, ri_user_time),
            "ri_system_time": std::mem::offset_of!(RUsageInfoV4, ri_system_time),
            "ri_resident_size": std::mem::offset_of!(RUsageInfoV4, ri_resident_size),
            "ri_phys_footprint": std::mem::offset_of!(RUsageInfoV4, ri_phys_footprint),
            "ri_proc_start_abstime": std::mem::offset_of!(RUsageInfoV4, ri_proc_start_abstime),
            "ri_proc_exit_abstime": std::mem::offset_of!(RUsageInfoV4, ri_proc_exit_abstime),
            "ri_diskio_bytesread": std::mem::offset_of!(RUsageInfoV4, ri_diskio_bytesread),
            "ri_diskio_byteswritten": std::mem::offset_of!(RUsageInfoV4, ri_diskio_byteswritten)
        },
        "bsd_size": std::mem::size_of::<BSDInfo>(),
        "bsd_offsets": { "pbi_pid": std::mem::offset_of!(BSDInfo, pbi_pid), "pbi_ppid": std::mem::offset_of!(BSDInfo, pbi_ppid) },
        "dependency":"libproc 0.14.11 fixed safe API types; verify against local public SDK independently",
        "cpu_unit":"raw proc_pid_rusage counters; no conversion or cross-platform unit claim"
    })
}
#[cfg(not(target_os = "macos"))]
fn abi() -> serde_json::Value {
    serde_json::json!({"status":"unsupported_platform"})
}
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Request {
    sequence: u64,
    pid: u32,
    expected: Option<Identity>,
}
const INPUT_BYTES: usize = 256;
const MAX_SAMPLES: u64 = 8192;
fn line(reader: &mut impl Read, bytes: &mut [u8; INPUT_BYTES]) -> Result<Option<usize>, String> {
    for used in 0..INPUT_BYTES {
        let n = reader.read(&mut bytes[used..=used]).map_err(err)?;
        if n == 0 {
            return if used == 0 {
                Ok(None)
            } else {
                Err("native_observer_partial_request".into())
            };
        }
        if bytes[used] == b'\n' {
            return Ok(Some(used));
        }
    }
    Err("native_observer_request_capacity".into())
}
fn err(error: impl std::fmt::Display) -> String {
    error.to_string()
}
fn save(writer: &mut impl Write, value: &impl Serialize) -> Result<(), String> {
    serde_json::to_writer(&mut *writer, value).map_err(err)?;
    writer.write_all(b"\n").map_err(err)?;
    writer.flush().map_err(err)
}
/// Bounded interactive probe, not the formal observer. Caller retains independent raw measurements.
/// EOF is normal only at a complete request boundary. No PID enumeration or memory-credit protocol.
pub fn probe(root: &Path) -> Result<(), String> {
    fs::create_dir(root).map_err(err)?;
    crate::heap_meter::activate_role(crate::diagnostic_contract::OBSERVER).map_err(err)?;
    let origin = Instant::now();
    let own = capture(std::process::id(), None, origin).map_err(|e| format!("{e:?}"))?;
    let mut stdout = std::io::stdout().lock();
    save(
        &mut stdout,
        &serde_json::json!({"schema":"mycity-native-observer-ready-v1","source_sha256":env!("MYCITY_SOURCE_SHA256"),"identity":own.identity,"abi":abi(),"input_bytes":INPUT_BYTES,"maximum_samples":MAX_SAMPLES,"arena":crate::heap_meter::arena_snapshot()}),
    )?;
    let mut file = std::io::BufWriter::with_capacity(
        64 * 1024,
        File::create_new(root.join("samples.jsonl")).map_err(err)?,
    );
    let mut stdin = std::io::stdin().lock();
    let mut input = [0; INPUT_BYTES];
    let mut samples = 0;
    let mut unknowns = 0;
    while let Some(n) = line(&mut stdin, &mut input)? {
        if samples == MAX_SAMPLES {
            return Err("native_observer_sample_capacity".into());
        }
        let request: Request = serde_json::from_slice(&input[..n]).map_err(err)?;
        if request.sequence != samples + 1 {
            return Err("native_observer_request_sequence".into());
        }
        let result = capture(request.pid, request.expected, origin);
        unknowns += u64::from(result.is_err());
        #[derive(Serialize)]
        struct Reply {
            sequence: u64,
            sample: Option<Sample>,
            unknown: Option<Unknown>,
        }
        let reply = Reply {
            sequence: request.sequence,
            sample: result.ok(),
            unknown: result.err(),
        };
        save(&mut file, &reply)?;
        save(&mut stdout, &reply)?;
        samples += 1;
    }
    file.flush().map_err(err)?;
    file.get_ref().sync_all().map_err(err)?;
    let mut report = File::create_new(root.join("report.json")).map_err(err)?;
    save(
        &mut report,
        &serde_json::json!({"schema":"mycity-native-observer-probe-v1","source_sha256":env!("MYCITY_SOURCE_SHA256"),"samples":samples,"unknowns":unknowns,"arena":crate::heap_meter::arena_snapshot(),"rust_heap":crate::heap_meter::snapshot(),"scope":"single PID capture backend; no enumeration/system pressure/swap/disk guard/clock mapping/retirement, not the formal observer","memory_credit_bytes":0,"formal_acceptance":"not_accepted"}),
    )?;
    report.sync_all().map_err(err)
}
#[cfg(test)]
mod tests {
    use super::*;
    struct Fake {
        births: [u64; 2],
        parents: [u32; 2],
        exit: u64,
        reads: usize,
        relations: usize,
        fail: bool,
    }
    impl Default for Fake {
        fn default() -> Self {
            Self {
                births: [12, 12],
                parents: [7, 7],
                exit: 0,
                reads: 0,
                relations: 0,
                fail: false,
            }
        }
    }
    impl Reader for Fake {
        fn usage(&mut self, _: u32) -> Result<Usage, Unknown> {
            if self.fail {
                return Err(Unknown::ResourceUnavailable);
            }
            let birth = self.births[self.reads];
            self.reads += 1;
            Ok(Usage {
                start_abstime: birth,
                exit_abstime: self.exit,
                resident_bytes: 100,
                footprint_bytes: 99,
                user_time_raw: 0,
                system_time_raw: 0,
                disk_read_bytes: 0,
                disk_written_bytes: 0,
            })
        }
        fn parent(&mut self, _: u32) -> Result<u32, Unknown> {
            let parent = self.parents[self.relations];
            self.relations += 1;
            Ok(parent)
        }
    }
    #[test]
    fn changed_birth_parent_exit_and_missing_are_unknown_never_zero() {
        let mut fake = Fake {
            births: [12, 13],
            ..Fake::default()
        };
        assert_eq!(
            checked_capture(&mut fake, 3, None, || 0).unwrap_err(),
            Unknown::IdentityChanged
        );
        let mut fake = Fake {
            parents: [7, 8],
            ..Fake::default()
        };
        assert_eq!(
            checked_capture(&mut fake, 3, None, || 0).unwrap_err(),
            Unknown::IdentityChanged
        );
        let mut fake = Fake {
            exit: 1,
            ..Fake::default()
        };
        assert_eq!(
            checked_capture(&mut fake, 3, None, || 0).unwrap_err(),
            Unknown::Exited
        );
        let mut fake = Fake {
            fail: true,
            ..Fake::default()
        };
        assert_eq!(
            checked_capture(&mut fake, 3, None, || 0).unwrap_err(),
            Unknown::ResourceUnavailable
        );
        assert_eq!(
            checked_capture(&mut Fake::default(), 0, None, || 0).unwrap_err(),
            Unknown::InvalidPid
        );
    }
    #[test]
    fn expected_identity_includes_mach_birth_and_parent() {
        let mut fake = Fake::default();
        let sample = checked_capture(&mut fake, 3, None, || 1).unwrap();
        assert_eq!(
            sample.identity,
            Identity {
                pid: 3,
                start_abstime: 12,
                parent_pid: 7
            }
        );
        for expected in [
            Identity {
                pid: 4,
                ..sample.identity
            },
            Identity {
                start_abstime: 13,
                ..sample.identity
            },
            Identity {
                parent_pid: 8,
                ..sample.identity
            },
        ] {
            assert_eq!(
                checked_capture(&mut Fake::default(), 3, Some(expected), || 1).unwrap_err(),
                Unknown::ExpectedIdentityMismatch
            );
        }
    }
    #[test]
    fn request_boundary_is_fixed_and_partial_eof_is_failure() {
        let mut buffer = [0; INPUT_BYTES];
        assert_eq!(line(&mut &b"{}\n"[..], &mut buffer).unwrap(), Some(2));
        assert!(line(&mut &b"{}"[..], &mut buffer).is_err());
        assert!(line(&mut &[b'x'; INPUT_BYTES][..], &mut buffer).is_err());
        assert_eq!(line(&mut &b""[..], &mut buffer).unwrap(), None);
        assert!(serde_json::from_slice::<Request>(
            br#"{"sequence":1,"pid":1,"expected":null,"extra":true}"#
        )
        .is_err());
    }
}
