//! Fixed warmup/measurement windows. Short diagnostics cannot accept formal steady.
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use std::{
    fs::{self, File},
    io::{Read, Write},
    path::Path,
    sync::atomic::{AtomicU8, Ordering},
    time::{Duration, Instant},
};
mod driver;
mod supervisor;
pub use driver::run as driver;
pub use supervisor::run;
const STOP: &str = "steady_safe_case_prefix_stop";
static CASE_STOP: AtomicU8 = AtomicU8::new(0);
pub fn case_boundary() -> Result<(), String> {
    match CASE_STOP.load(Ordering::Acquire) {
        0 => Ok(()),
        1 => Err(STOP.into()),
        _ => Err("steady_case_owner_lost".into()),
    }
}
/// Only used by dedicated prototype children. EOF is failure, never a passed case.
pub fn watch_case_owner() -> Result<(), String> {
    let mut b = [0];
    std::io::stdin().read_exact(&mut b).map_err(err)?;
    if b != [b'1'] {
        return Err("steady_case_handshake".into());
    }
    std::thread::Builder::new()
        .name("steady-case-owner".into())
        .stack_size(64 * 1024)
        .spawn(|| {
            let mut b = [0];
            let ok = std::io::stdin().read_exact(&mut b).is_ok() && b == [b'S'];
            CASE_STOP.store(if ok { 1 } else { 2 }, Ordering::Release);
        })
        .map_err(err)?;
    Ok(())
}
pub fn case_ready(root: &Path) -> Result<(), String> {
    let clock = crate::heap_meter::clock_reading();
    save(&root.join("case-child-ready.json"), &json!({"clock":clock}))
}
pub fn case_outcome(path: &Path, outcome: Result<Value, String>) -> Result<(), String> {
    let summary = match &outcome {
        Ok(_) => json!({"completed_case":true,"safe_prefix":false}),
        Err(e) if e == STOP => json!({"completed_case":false,"safe_prefix":true}),
        Err(e) => return Err(e.clone()),
    };
    let result = match outcome {
        Ok(result) => save(
            path,
            &json!({"completed_case":true,"safe_prefix":false,"result":result,"source_sha256":env!("MYCITY_SOURCE_SHA256")}),
        ),
        Err(e) if e == STOP => save(
            path,
            &json!({"completed_case":false,"safe_prefix":true,"correctness_case_passed":false,"last_transaction":"durable boundary; no in-flight write interrupted","source_sha256":env!("MYCITY_SOURCE_SHA256")}),
        ),
        Err(e) => Err(e),
    };
    result?;
    save(&path.with_file_name("case-summary.json"), &summary)
}
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Scenario {
    L0,
    L1,
    L2,
}
impl Scenario {
    pub fn parse(s: &str) -> Result<Self, String> {
        match s {
            "l0" => Ok(Self::L0),
            "l1" => Ok(Self::L1),
            "l2" => Ok(Self::L2),
            _ => Err("steady_scenario".into()),
        }
    }
    pub fn name(self) -> &'static str {
        match self {
            Self::L0 => "l0",
            Self::L1 => "l1",
            Self::L2 => "l2",
        }
    }
}
#[derive(Clone, Copy, Debug)]
pub struct Plan {
    pub formal: bool,
    pub warmup: Duration,
    pub measure: Duration,
    pub quiesce: Duration,
}
impl Plan {
    pub fn parse(s: &str) -> Result<Self, String> {
        match s {
            "formal" => Ok(Self {
                formal: true,
                warmup: Duration::from_secs(300),
                measure: Duration::from_secs(1800),
                quiesce: Duration::from_secs(10),
            }),
            "smoke" => Ok(Self {
                formal: false,
                warmup: Duration::from_secs(3),
                measure: Duration::from_secs(12),
                quiesce: Duration::from_secs(1),
            }),
            _ => Err("steady_mode".into()),
        }
    }
    pub fn name(self) -> &'static str {
        if self.formal {
            "formal"
        } else {
            "smoke"
        }
    }
    pub fn total(self) -> Duration {
        self.warmup + self.measure
    }
    fn admission(self, elapsed: Duration) -> bool {
        elapsed < self.warmup - self.quiesce
    }
}
pub(super) fn err(e: impl std::fmt::Display) -> String {
    e.to_string()
}
pub(super) fn save(path: &Path, v: &impl Serialize) -> Result<(), String> {
    let mut f = File::create_new(path).map_err(err)?;
    serde_json::to_writer_pretty(&mut f, v).map_err(err)?;
    f.write_all(b"\n").map_err(err)?;
    f.sync_all().map_err(err)
}
pub(super) fn load(path: &Path) -> Result<Value, String> {
    let b = mycity_core::read_bounded(File::open(path).map_err(err)?, 65536).map_err(err)?;
    serde_json::from_slice(&b).map_err(err)
}
struct Events {
    began: Instant,
    file: File,
    bytes: usize,
}
impl Events {
    fn new(path: &Path) -> Result<Self, String> {
        Ok(Self {
            began: Instant::now(),
            file: File::create_new(path).map_err(err)?,
            bytes: 0,
        })
    }
    fn elapsed(&self) -> Duration {
        self.began.elapsed()
    }
    fn write(&mut self, kind: &str, data: Value) -> Result<(), String> {
        let b = serde_json::to_vec(
            &json!({"elapsed_ns":self.elapsed().as_nanos(),"kind":kind,"data":data}),
        )
        .map_err(err)?;
        if b.len() > 65536 || self.bytes + b.len() + 1 > 8 * 1024 * 1024 {
            return Err("steady_event_capacity".into());
        }
        self.file
            .write_all(&b)
            .and_then(|_| self.file.write_all(b"\n"))
            .map_err(err)?;
        self.file.flush().map_err(err)?;
        self.bytes += b.len() + 1;
        Ok(())
    }
    fn wait(&self, at: Duration) {
        while self.elapsed() < at {
            std::thread::sleep(
                at.saturating_sub(self.elapsed())
                    .min(Duration::from_millis(20)),
            )
        }
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn formal_boundaries_never_shift_and_smoke_cannot_accept() {
        let p = Plan::parse("formal").unwrap();
        assert_eq!(p.total().as_secs(), 2100);
        assert!(p.admission(Duration::from_secs(289)));
        assert!(!p.admission(Duration::from_secs(290)));
        assert!(!Plan::parse("smoke").unwrap().formal);
        assert!(Plan::parse("2100").is_err());
    }
}
