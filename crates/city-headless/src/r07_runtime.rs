//! Ephemeral, source/nonce/birth-bound physical readings; never city state.
use crate::{control_data::Text, native_observer::Identity};
use serde::{Deserialize, Serialize};
#[cfg(target_os = "macos")]
use std::{fs::File, path::Path};
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Role {
    Observer,
    Supervisor,
    Driver,
}
#[derive(Clone, Copy, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Component {
    pub role: Role,
    pub identity: Identity,
    pub footprint_bytes: u64,
}
#[derive(Clone, Copy, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Reading {
    pub schema: u32,
    pub source: Text<64>,
    pub nonce: Text<32>,
    pub sequence: u64,
    pub monotonic_begin_ns: u64,
    pub monotonic_end_ns: u64,
    pub components: [Option<Component>; 3],
}
#[derive(Clone, Copy, Debug, Serialize)]
pub struct Decision {
    pub sequence: u64,
    pub permitted_bytes: usize,
    pub reason: &'static str,
}
impl Decision {
    pub fn zero(reason: &'static str) -> Self {
        Self {
            sequence: 0,
            permitted_bytes: 0,
            reason,
        }
    }
}
#[cfg(target_os = "macos")]
pub fn monotonic_ns() -> Result<u64, String> {
    let t = rustix::time::clock_gettime(rustix::time::ClockId::Monotonic);
    let sec = u64::try_from(t.tv_sec).map_err(|_| "r07_clock_seconds")?;
    let ns = u64::try_from(t.tv_nsec).map_err(|_| "r07_clock_nanos")?;
    if ns >= 1_000_000_000 {
        return Err("r07_clock_nanos".into());
    }
    sec.checked_mul(1_000_000_000)
        .and_then(|n| n.checked_add(ns))
        .ok_or_else(|| "r07_clock_overflow".into())
}
#[cfg(target_os = "macos")]
fn evaluate(reading: Reading, nonce: &str, minimum_sequence: u64) -> Result<Decision, String> {
    let now = monotonic_ns()?;
    if reading.schema != 1
        || reading.source.as_str() != env!("MYCITY_SOURCE_SHA256")
        || reading.nonce.as_str() != nonce
        || reading.sequence == 0
        || reading.sequence < minimum_sequence
        || reading.monotonic_begin_ns > reading.monotonic_end_ns
        || reading.monotonic_end_ns > now
        || now - reading.monotonic_begin_ns > 1_000_000_000
    {
        return Ok(Decision::zero("reading_identity_sequence_or_ttl"));
    }
    let mut total = 0usize;
    for (i, c) in reading.components.iter().enumerate() {
        let Some(c) = c else { continue };
        if c.role != [Role::Observer, Role::Supervisor, Role::Driver][i] {
            return Ok(Decision::zero("component_role_slot"));
        }
        if reading.components[..i]
            .iter()
            .flatten()
            .any(|p| p.identity.pid == c.identity.pid)
        {
            return Ok(Decision::zero("duplicate_process_identity"));
        }
        let Ok(current) = crate::native_observer::capture(
            c.identity.pid,
            Some(c.identity),
            std::time::Instant::now(),
        ) else {
            continue;
        };
        let measured = c.footprint_bytes.min(current.usage.footprint_bytes);
        total = total
            .checked_add(usize::try_from(measured).map_err(|_| "r07_reading_size")?)
            .ok_or("r07_reading_sum")?;
    }
    Ok(Decision {
        sequence: reading.sequence,
        permitted_bytes: total,
        reason: "fresh_unique_live_same_use_components",
    })
}
/// Missing, invalid, stale or exited components revoke credit: caller must fill
/// suffix before acknowledging. This is not "keep the previous valid maximum".
pub fn current(minimum_sequence: u64) -> Result<Decision, String> {
    #[cfg(target_os = "macos")]
    {
        let Some(path) = std::env::var_os("MYCITY_R07_RUNTIME") else {
            return Ok(Decision::zero("no_provider"));
        };
        let Some(nonce) = std::env::var_os("MYCITY_PROCESS_NONCE") else {
            return Ok(Decision::zero("no_nonce"));
        };
        let Some(nonce) = nonce.to_str() else {
            return Ok(Decision::zero("bad_nonce"));
        };
        let path = Path::new(&path);
        if !path.is_absolute() {
            return Err("r07_provider_path".into());
        }
        let bytes = match File::open(path)
            .map_err(|e| e.to_string())
            .and_then(|file| mycity_core::read_bounded(file, 4096).map_err(|e| e.to_string()))
        {
            Ok(bytes) => bytes,
            Err(_) => return Ok(Decision::zero("provider_unavailable")),
        };
        let reading: Reading = match serde_json::from_slice(&bytes) {
            Ok(r) => r,
            Err(_) => return Ok(Decision::zero("provider_invalid")),
        };
        evaluate(reading, nonce, minimum_sequence)
    }
    #[cfg(not(target_os = "macos"))]
    {
        let _ = minimum_sequence;
        Ok(Decision::zero("native_provider_unavailable"))
    }
}

#[cfg(all(test, target_os = "macos"))]
mod tests {
    use super::*;
    fn live() -> Reading {
        let sample =
            crate::native_observer::capture(std::process::id(), None, std::time::Instant::now())
                .unwrap();
        let now = monotonic_ns().unwrap();
        Reading {
            schema: 1,
            source: Text::new(env!("MYCITY_SOURCE_SHA256")).unwrap(),
            nonce: Text::new("0123456789abcdef0123456789abcdef").unwrap(),
            sequence: 7,
            monotonic_begin_ns: now,
            monotonic_end_ns: now,
            components: [
                Some(Component {
                    role: Role::Observer,
                    identity: sample.identity,
                    footprint_bytes: sample.usage.footprint_bytes,
                }),
                None,
                None,
            ],
        }
    }
    #[test]
    fn birth_nonce_sequence_ttl_and_unique_process_gate_real_readings() {
        let r = live();
        let nonce = r.nonce.as_str();
        assert!(evaluate(r, nonce, 7).unwrap().permitted_bytes > 0);
        assert_eq!(evaluate(r, "wrong", 7).unwrap().permitted_bytes, 0);
        assert_eq!(evaluate(r, nonce, 8).unwrap().permitted_bytes, 0);
        let mut bad = r;
        bad.monotonic_begin_ns = 0;
        assert_eq!(evaluate(bad, nonce, 7).unwrap().permitted_bytes, 0);
        let mut bad = r;
        bad.components[0].as_mut().unwrap().identity.start_abstime += 1;
        assert_eq!(evaluate(bad, nonce, 7).unwrap().permitted_bytes, 0);
        let mut duplicate = r;
        let mut second = r.components[0].unwrap();
        second.role = Role::Supervisor;
        duplicate.components[1] = Some(second);
        assert_eq!(evaluate(duplicate, nonce, 7).unwrap().permitted_bytes, 0);
        let mut lower = r;
        lower.components[0].as_mut().unwrap().footprint_bytes = 1;
        assert_eq!(evaluate(lower, nonce, 7).unwrap().permitted_bytes, 1);
    }
}
