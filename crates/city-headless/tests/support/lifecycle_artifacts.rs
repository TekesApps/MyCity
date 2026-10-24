//! Optional retained finite-validation roots. Normal standalone test behavior remains local.
use std::path::{Path, PathBuf};
pub fn root() -> PathBuf {
    std::env::var_os("MYCITY_LIFECYCLE_TEST_ARTIFACT_ROOT").map(PathBuf::from).unwrap_or_else(std::env::temp_dir)
}
pub fn cleanup(path: impl AsRef<Path>) -> std::io::Result<()> {
    if std::env::var_os("MYCITY_LIFECYCLE_TEST_ARTIFACT_ROOT").is_some() || std::thread::panicking() { Ok(()) }
    else { std::fs::remove_dir_all(path) }
}
pub fn exe() -> &'static Path {
    static EXE: std::sync::OnceLock<PathBuf> = std::sync::OnceLock::new();
    EXE.get_or_init(|| std::env::var_os("MYCITY_PROTOCOL_WORKER_EXE").map(PathBuf::from)
        .unwrap_or_else(|| PathBuf::from(env!("CARGO_BIN_EXE_mycity-headless")))).as_path()
}
