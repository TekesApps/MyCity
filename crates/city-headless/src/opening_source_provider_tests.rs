use super::{
    load_resources_file, read_local_bytes, read_local_file, MAX_LOCAL_BYTES_LIMIT,
    MAX_RESOURCE_MANIFEST_BYTES, LOCAL_FILE_SOURCE_LABEL, RESOURCE_MANIFEST_SCHEMA,
};
use std::{
    fs::{self, File, OpenOptions},
    io::Write,
    path::PathBuf,
    process::Command,
    sync::atomic::{AtomicU64, Ordering},
    thread,
    time::{Duration, Instant},
};

static NEXT_ID: AtomicU64 = AtomicU64::new(1);

fn test_base() -> PathBuf {
    std::env::var_os("MYCITY_OPENING_SOURCE_TEST_ROOT").map(PathBuf::from)
        .unwrap_or_else(std::env::temp_dir)
}
fn cleanup_file(path: impl AsRef<std::path::Path>) -> std::io::Result<()> {
    if std::env::var_os("MYCITY_OPENING_SOURCE_TEST_ROOT").is_some() || std::thread::panicking() { Ok(()) }
    else { fs::remove_file(path) }
}

fn temp_path(suffix: &str) -> PathBuf {
    test_base().join(format!(
        "mycity-opening-source-{}-{}-{suffix}",
        std::process::id(),
        NEXT_ID.fetch_add(1, Ordering::Relaxed)
    ))
}

#[test]
fn reads_selected_regular_file_and_reports_opened_bytes_and_identity() {
    let path = temp_path("valid.json");
    let bytes = include_bytes!("../../../fixtures/m0-bootstrap.json");
    fs::write(&path, bytes).unwrap();

    let loaded = read_local_file(&path).unwrap();
    assert_eq!(loaded.provenance().source_label, LOCAL_FILE_SOURCE_LABEL);
    assert_eq!(loaded.provenance().bytes_read, bytes.len());
    assert_eq!(loaded.provenance().sha256, loaded.input().source().sha256());
    assert_eq!(loaded.input().source().bytes(), bytes);
    assert_eq!(loaded.provenance().file_identity.length as usize, bytes.len());
    assert!(loaded.provenance().canonical_path.is_absolute());
    assert_eq!(
        loaded.provenance().to_json()["source_label"].as_str(),
        Some(LOCAL_FILE_SOURCE_LABEL)
    );
    let _ = cleanup_file(path);
}

#[test]
fn rejects_empty_nonregular_oversized_and_missing_paths() {
    let empty = temp_path("empty.json");
    File::create(&empty).unwrap();
    assert_eq!(read_local_file(&empty).unwrap_err(), "opening_source_empty_file");
    let _ = cleanup_file(empty);

    assert_eq!(
        read_local_file(test_base()).unwrap_err(),
        "opening_source_not_regular_file"
    );

    let oversized = temp_path("oversized.json");
    let file = File::create(&oversized).unwrap();
    file.set_len(super::INPUT_LIMIT + 1).unwrap();
    assert_eq!(read_local_file(&oversized).unwrap_err(), "input_too_large");
    let _ = cleanup_file(oversized);

    assert_eq!(
        read_local_file(temp_path("does-not-exist.json")).unwrap_err(),
        "opening_source_open_failed"
    );
}

#[cfg(unix)]
#[test]
fn rejects_fifo_without_waiting_for_a_writer() {
    let path = temp_path("fifo.json");
    let result = Command::new("/usr/bin/mkfifo").arg(&path).status();
    assert!(result.expect("mkfifo must be available for this Unix source test").success());

    // Isolate the attempted open in a child so a regression cannot hang the parent test process.
    let mut child = Command::new(std::env::current_exe().unwrap())
        .arg("fifo_open_probe_child")
        .arg("--ignored")
        .arg("--nocapture")
        .env("MYCITY_OPENING_FIFO_PROBE", &path)
        .spawn()
        .unwrap();
    let deadline = Instant::now() + Duration::from_secs(5);
    loop {
        match child.try_wait().unwrap() {
            Some(status) => {
                assert!(status.success(), "FIFO child probe failed: {status}");
                break;
            }
            None if Instant::now() < deadline => thread::sleep(Duration::from_millis(10)),
            None => {
                child.kill().unwrap();
                let _ = child.wait();
                panic!("FIFO provider probe exceeded its bounded wait");
            }
        }
    }
    let _ = cleanup_file(path);
}

#[cfg(unix)]
#[test]
#[ignore = "explicitly invoked by the bounded parent FIFO test"]
fn fifo_open_probe_child() {
    let Ok(path) = std::env::var("MYCITY_OPENING_FIFO_PROBE") else {
        return;
    };
    assert_eq!(
        read_local_file(path).unwrap_err(),
        "opening_source_not_regular_file"
    );
}

#[test]
fn preserves_exact_invalid_or_future_schema_bytes_for_existing_assembly_gate() {
    for (suffix, bytes) in [
        ("invalid.json", b"{".as_slice()),
        ("future.json", br#"{"schema":"mycity-opening-v999"}"#.as_slice()),
    ] {
        let path = temp_path(suffix);
        let mut file = OpenOptions::new()
            .write(true)
            .create_new(true)
            .open(&path)
            .unwrap();
        file.write_all(bytes).unwrap();
        drop(file);
        let loaded = read_local_file(&path).unwrap();
        assert_eq!(loaded.input().source().bytes(), bytes);
        assert_eq!(loaded.provenance().sha256, loaded.input().source().sha256());
        let _ = cleanup_file(path);
    }
}

#[test]
fn rejects_paths_over_the_declared_provenance_bound() {
    let long_path = PathBuf::from(format!("/{}", "x".repeat(4096)));
    assert_eq!(
        read_local_file(long_path).unwrap_err(),
        "opening_source_path_too_large"
    );
}

#[test]
fn resource_manifest_is_versioned_bounded_and_rejects_unknown_or_duplicate_fields() {
    let path = temp_path("resources.json");
    fs::write(
        &path,
        format!(
            r#"{{"schema":"{RESOURCE_MANIFEST_SCHEMA}","resources":[{{"id":7,"capacity":11}}]}}"#
        ),
    )
    .unwrap();
    let loaded = load_resources_file(&path).unwrap();
    assert_eq!(loaded.resources.len(), 1);
    assert_eq!((loaded.resources[0].id, loaded.resources[0].capacity), (7, 11));
    assert_eq!((loaded.resources[0].reserved, loaded.resources[0].occupied), (0, 0));
    let _ = cleanup_file(&path);

    for (suffix, json) in [
        (
            "unknown.json",
            format!(r#"{{"schema":"{RESOURCE_MANIFEST_SCHEMA}","resources":[{{"id":7,"capacity":11,"occupied":1}}]}}"#),
        ),
        (
            "reserved.json",
            format!(r#"{{"schema":"{RESOURCE_MANIFEST_SCHEMA}","resources":[{{"id":7,"capacity":11,"reserved":1}}]}}"#),
        ),
        (
            "duplicate.json",
            format!(r#"{{"schema":"{RESOURCE_MANIFEST_SCHEMA}","resources":[{{"id":7,"capacity":11}},{{"id":7,"capacity":12}}]}}"#),
        ),
        (
            "zero.json",
            format!(r#"{{"schema":"{RESOURCE_MANIFEST_SCHEMA}","resources":[{{"id":0,"capacity":11}}]}}"#),
        ),
        (
            "future.json",
            r#"{"schema":"mycity-resources-v999","resources":[]}"#.to_owned(),
        ),
    ] {
        let path = temp_path(suffix);
        fs::write(&path, json).unwrap();
        assert!(load_resources_file(&path).is_err());
        let _ = cleanup_file(path);
    }

    let too_many = format!(
        r#"{{"schema":"{RESOURCE_MANIFEST_SCHEMA}","resources":[{}]}}"#,
        (1..=17)
            .map(|id| format!(r#"{{"id":{id},"capacity":1}}"#))
            .collect::<Vec<_>>()
            .join(",")
    );
    assert!(too_many.len() < MAX_RESOURCE_MANIFEST_BYTES);
    let path = temp_path("too-many.json");
    fs::write(&path, too_many).unwrap();
    assert_eq!(
        load_resources_file(&path).unwrap_err(),
        "resource_manifest_count_limit"
    );
    let _ = cleanup_file(path);

    let explicit_empty = format!(r#"{{"schema":"{RESOURCE_MANIFEST_SCHEMA}","resources":[]}}"#);
    let path = temp_path("empty-resources.json");
    fs::write(&path, explicit_empty).unwrap();
    assert!(load_resources_file(&path).unwrap().resources.is_empty());
    let _ = cleanup_file(path);

    let path = temp_path("resource-limit.json");
    fs::write(&path, vec![b' '; MAX_RESOURCE_MANIFEST_BYTES + 1]).unwrap();
    assert_eq!(
        load_resources_file(&path).unwrap_err(),
        "resource_manifest_too_large"
    );
    let _ = cleanup_file(path);
}

#[test]
fn local_bytes_uses_one_bounded_buffer_for_spatial_assets() {
    let path = temp_path("spatial.png");
    let bytes = b"single-buffer spatial bytes";
    fs::write(&path, bytes).unwrap();
    let loaded = read_local_bytes(&path, MAX_LOCAL_BYTES_LIMIT).unwrap();
    assert_eq!(loaded.bytes(), bytes);
    assert_eq!(loaded.provenance().bytes_read, bytes.len());
    assert_eq!(loaded.provenance().sha256, mycity_core::sha256(bytes));
    let _ = cleanup_file(path);

    let oversized = temp_path("bounded.json");
    fs::write(&oversized, b"12345").unwrap();
    assert_eq!(
        read_local_bytes(&oversized, 4).unwrap_err(),
        "input_too_large"
    );
    let _ = cleanup_file(oversized);
}
