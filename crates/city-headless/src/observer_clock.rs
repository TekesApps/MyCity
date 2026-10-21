//! Optional startup-only causal exchange; never assumes cross-process Instant epochs.
use serde::Deserialize;
use std::{
    fs::{self, File},
    io::Write,
    path::Path,
    time::{Duration, Instant},
};
const SOURCE: &str = env!("MYCITY_SOURCE_SHA256");
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Request<'a> {
    schema: &'a str,
    pid: u32,
    source_sha256: &'a str,
    nonce: &'a str,
}
fn save(path: &Path, value: &impl serde::Serialize) -> Result<(), String> {
    let mut file = File::create_new(path).map_err(|e| e.to_string())?;
    serde_json::to_writer(&mut file, value).map_err(|e| e.to_string())?;
    file.write_all(b"\n").map_err(|e| e.to_string())?;
    file.sync_all().map_err(|e| e.to_string())
}
fn valid(request: &Request<'_>) -> bool {
    request.schema == "mycity-observer-clock-request-v1"
        && request.pid == std::process::id()
        && request.source_sha256 == SOURCE
        && request.nonce.len() == 32
        && request.nonce.bytes().all(|b| b.is_ascii_hexdigit())
}
pub(crate) fn synchronize(root: &Path, elapsed: impl Fn() -> u64) -> Result<(), String> {
    match std::env::var_os("MYCITY_OBSERVER_CLOCK") {
        None => return Ok(()),
        Some(v) if v == "1" => (),
        Some(_) => return Err("observer_clock_mode".into()),
    }
    exchange(root, elapsed, Duration::from_secs(10))
}
fn exchange(root: &Path, elapsed: impl Fn() -> u64, timeout: Duration) -> Result<(), String> {
    save(
        &root.join("observer-clock-ready.json"),
        &serde_json::json!({
            "schema":"mycity-observer-clock-ready-v1","pid":std::process::id(),"source_sha256":SOURCE
        }),
    )?;
    let began = Instant::now();
    let path = root.join("observer-clock-request.json");
    while !path.exists() {
        if began.elapsed() >= timeout {
            return Err("observer_clock_timeout".into());
        }
        std::thread::sleep(Duration::from_millis(1));
    }
    if !fs::symlink_metadata(&path)
        .map_err(|e| e.to_string())?
        .file_type()
        .is_file()
    {
        return Err("observer_clock_request_file".into());
    }
    let bytes = mycity_core::read_bounded(File::open(path).map_err(|e| e.to_string())?, 2048)
        .map_err(|e| e.to_string())?;
    let request: Request<'_> = serde_json::from_slice(&bytes).map_err(|e| e.to_string())?;
    if !valid(&request) {
        return Err("observer_clock_identity".into());
    }
    // Receive follows the observer's send; this timestamp precedes its reply read.
    // Both file/durability costs stay within the raw send/receive bracket.
    let elapsed_ns = elapsed();
    save(
        &root.join("observer-clock-reply.json"),
        &serde_json::json!({
            "schema":"mycity-observer-clock-reply-v1","pid":std::process::id(),
            "source_sha256":SOURCE,"nonce":request.nonce,"elapsed_ns":elapsed_ns
        }),
    )
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn source_pid_nonce_and_unknown_fields_cannot_authorize_clock_reply() {
        let nonce = "0123456789abcdef0123456789abcdef";
        let mut req = Request {
            schema: "mycity-observer-clock-request-v1",
            pid: std::process::id(),
            source_sha256: SOURCE,
            nonce,
        };
        assert!(valid(&req));
        req.pid ^= 1;
        assert!(!valid(&req));
        req.pid ^= 1;
        req.source_sha256 = "wrong";
        assert!(!valid(&req));
        req.source_sha256 = SOURCE;
        req.nonce = "short";
        assert!(!valid(&req));
        req.nonce = "zzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzz";
        assert!(!valid(&req));
        let bytes=serde_json::to_vec(&serde_json::json!({"schema":req.schema,"pid":req.pid,"source_sha256":SOURCE,"nonce":nonce,"extra":true})).unwrap();
        assert!(serde_json::from_slice::<Request<'_>>(&bytes).is_err());
    }
    #[test]
    fn absent_request_times_out_without_a_reply_or_model_start() {
        let root =
            std::env::temp_dir().join(format!("mc-observer-clock-timeout-{}", std::process::id()));
        fs::create_dir(&root).unwrap();
        assert!(
            matches!(exchange(&root,||123,Duration::from_millis(5)),Err(e) if e=="observer_clock_timeout")
        );
        assert!(!root.join("observer-clock-reply.json").exists());
        fs::remove_dir_all(root).unwrap();
    }
}
