//! Same nonce-bound causal startup exchange as the original observer.
use super::{err, now, save};
use serde::Deserialize;
use std::{
    fs::{self, File},
    io::{Read, Write},
    path::Path,
    process::Child,
    time::{Duration, Instant},
};
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Ready {
    schema: String,
    pid: u32,
    source_sha256: String,
}
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Reply {
    schema: String,
    pid: u32,
    source_sha256: String,
    nonce: String,
    elapsed_ns: u64,
}
fn read(path: &Path) -> Result<Option<Vec<u8>>, String> {
    let file = match File::open(path) {
        Ok(f) => f,
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => return Ok(None),
        Err(e) => return Err(err(e)),
    };
    if !file.metadata().map_err(err)?.is_file() {
        return Err("native_clock_file_kind".into());
    }
    let raw = mycity_core::read_bounded(file, 2048).map_err(err)?;
    Ok(raw.ends_with(b"\n").then_some(raw))
}
fn wait(child: &mut Child, path: &Path, deadline: Instant) -> Result<Vec<u8>, String> {
    loop {
        if let Some(raw) = read(path)? {
            return Ok(raw);
        }
        if child.try_wait().map_err(err)?.is_some() {
            return Err("native_clock_peer_exited".into());
        }
        if Instant::now() >= deadline {
            return Err("native_clock_timeout".into());
        }
        std::thread::sleep(Duration::from_millis(1));
    }
}
pub(super) fn nonce() -> Result<String, String> {
    let mut b = [0; 16];
    File::open("/dev/urandom")
        .map_err(err)?
        .read_exact(&mut b)
        .map_err(err)?;
    const HEX: &[u8; 16] = b"0123456789abcdef";
    let mut out = String::with_capacity(32);
    for byte in b {
        out.push(char::from(HEX[(byte >> 4) as usize]));
        out.push(char::from(HEX[(byte & 15) as usize]));
    }
    Ok(out)
}
pub(super) fn exchange(
    child: &mut Child,
    root: &Path,
    origin: Instant,
    spawn_before: u64,
) -> Result<(), String> {
    let model = root.join("model");
    let deadline = Instant::now() + Duration::from_secs(10);
    let ready: Ready = serde_json::from_slice(&wait(
        child,
        &model.join("observer-clock-ready.json"),
        deadline,
    )?)
    .map_err(err)?;
    if ready.schema != "mycity-observer-clock-ready-v1"
        || ready.pid != child.id()
        || ready.source_sha256 != env!("MYCITY_SOURCE_SHA256")
    {
        return Err("native_clock_ready_identity".into());
    }
    let nonce = nonce()?;
    let sent = now(origin);
    let tmp = model.join("observer-clock-request.tmp");
    let mut file = File::create_new(&tmp).map_err(err)?;
    serde_json::to_writer(&mut file,&serde_json::json!({"schema":"mycity-observer-clock-request-v1","pid":child.id(),"source_sha256":env!("MYCITY_SOURCE_SHA256"),"nonce":nonce})).map_err(err)?;
    file.write_all(b"\n").map_err(err)?;
    file.sync_all().map_err(err)?;
    fs::rename(tmp, model.join("observer-clock-request.json")).map_err(err)?;
    let reply: Reply = serde_json::from_slice(&wait(
        child,
        &model.join("observer-clock-reply.json"),
        deadline,
    )?)
    .map_err(err)?;
    let received = now(origin);
    if reply.schema != "mycity-observer-clock-reply-v1"
        || reply.pid != child.id()
        || reply.source_sha256 != env!("MYCITY_SOURCE_SHA256")
        || reply.nonce != nonce
    {
        return Err("native_clock_reply_identity".into());
    }
    let (lo, hi) = bounds(spawn_before, sent, received, reply.elapsed_ns)?;
    save(
        &root.join("observer-clock-map.json"),
        &serde_json::json!({"supervisor_pid":child.id(),"source_sha256":env!("MYCITY_SOURCE_SHA256"),"supervisor_origin_observer_ns_min":lo,"supervisor_origin_observer_ns_max":hi,"uncertainty_ns":hi-lo,"method":"nonce-bound real request/reply; intersect original spawn lower bound","request_sent_observer_ns":sent,"reply_received_observer_ns":received,"spawn_before_observer_ns":spawn_before,"nonce":nonce}),
    )
}
fn bounds(spawn: u64, sent: u64, received: u64, elapsed: u64) -> Result<(i128, i128), String> {
    if spawn > sent || sent > received {
        return Err("native_clock_order".into());
    }
    let lo = i128::from(spawn).max(i128::from(sent) - i128::from(elapsed));
    let hi = i128::from(received) - i128::from(elapsed);
    if lo > hi {
        return Err("native_clock_empty_intersection".into());
    }
    Ok((lo, hi))
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn origin_bounds_do_not_assume_shared_instants() {
        assert_eq!(bounds(5, 20, 25, 10).unwrap(), (10, 15));
        assert!(bounds(5, 20, 25, 30).is_err());
        assert!(bounds(21, 20, 25, 10).is_err());
    }
}
