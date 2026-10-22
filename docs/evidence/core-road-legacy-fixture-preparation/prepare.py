#!/usr/bin/env python3
"""Prepare old business source plus a fixture-export test; never build or run it.

The new harness is not a previously retained v119 world. Actual fixture creation
must run later with the frozen old implementation in the exclusive build window.
"""
import hashlib
import json
from pathlib import Path, PurePosixPath
import subprocess
import zipfile

REPO = Path(__file__).resolve().parents[3]
HERE = Path(__file__).resolve().parent
OLD_COMMIT = "b4d6889e8424a829cba135fe293267c60cb94fcd"
OLD_EVIDENCE = REPO / "docs/evidence/core-transport-tax-integration-119/attempt02"
OLD_ARCHIVE_SHA256 = "75d641c5696873580e8ef2a92606d49d9e6fa6dfe4489b7439c9886e5cda7bbe"
OLD_MANIFEST_SHA256 = "3eaac12110dca5e07bd008fe31406eff03771b554faf040ef2cdb90b97c2c0f6"
OLD_BUSINESS_SHA256 = "717148eab1b00e5fc183ee240240fa9150cc839f9a94aa96aaa8bb8a22f67f82"
OUTPUT = REPO / "work/legacy-accepted-project-119-src"
FIXTURE_PREFIX = "docs/evidence/core-construction-compat/attempt-01/"
FIXTURE_PATHS = [FIXTURE_PREFIX + "old-v106-world/" + name for name in (
    "genesis.json", "CURRENT.json", "root-0.json", "root-1.json", "wal.bin",
)] + [FIXTURE_PREFIX + "old-v106-storage-probe.json"]

HARNESS = r'''// Fixture-export harness only. Included business/test source remains v119.
include!("construction.rs");

fn copy_closed_fixture_tree(source: &Path, destination: &Path) {
    fs::create_dir(destination).unwrap();
    for item in fs::read_dir(source).unwrap() {
        let item = item.unwrap();
        let kind = item.file_type().unwrap();
        let target = destination.join(item.file_name());
        if kind.is_dir() {
            copy_closed_fixture_tree(&item.path(), &target);
        } else {
            assert!(kind.is_file(), "fixture tree must not contain symlinks or sockets");
            fs::copy(item.path(), target).unwrap();
        }
    }
}

#[test]
fn export_v119_accepted_project_wal_and_checkpoint() {
    let output = PathBuf::from(std::env::var_os("MYCITY_LEGACY_PROJECT_OUT").unwrap());
    assert!(!output.exists(), "refuse to overwrite any fixture or failed run");
    fs::create_dir(&output).unwrap();
    // No Temp/Drop deletion: failed fixture generation keeps its full original.
    let path = output.join("original");
    let mut s = create(&path);
    approvals(&mut s);
    let op = start_id(&s);
    call(&mut s, Caller::Account(4), C::Start, op);
    ordinary_action(&mut s, Caller::System, Action::Timeline { command: T::Enable });
    const SHIFT: u64 = 17 * 3_600;
    advance(&mut s, SHIFT);
    let op = start_id(&s);
    call(&mut s, Caller::Account(3), C::Work {
        at: GameTime::new(SHIFT as i64).unwrap(),
        elapsed_seconds: SHIFT as u32,
        worker_seconds: 28_800,
    }, op);
    let op = start_id(&s);
    call(&mut s, Caller::Account(3), C::SubmitInspection, op);
    let op = start_id(&s);
    call(&mut s, Caller::Account(1), C::AcceptInspection {
        inspector: 1, accepted: true,
    }, op);
    let project = s.engine().unwrap().world().view().construction.unwrap();
    assert!(project.operating_published);
    let wal_hash = s.state_hash().unwrap();
    let wal_sequence = s.committed_sequence();
    drop(s);
    copy_closed_fixture_tree(&path, &output.join("accepted-wal"));

    let mut s = Store::open(&path, RULES).unwrap();
    assert_eq!(s.state_hash().unwrap(), wal_hash);
    s.checkpoint().unwrap();
    let checkpoint_hash = s.state_hash().unwrap();
    let checkpoint_sequence = s.committed_sequence();
    drop(s);
    copy_closed_fixture_tree(&path, &output.join("accepted-checkpoint"));
    let reopened = Store::open(&path, RULES).unwrap();
    assert_eq!(reopened.state_hash().unwrap(), checkpoint_hash);
    drop(reopened);
    fs::write(output.join("report.json"), serde_json::to_vec_pretty(&json!({
        "scope": "new fixtures executed by frozen v119 business implementation plus export-only harness",
        "rules": RULES,
        "project_id": project.id,
        "operating_published": project.operating_published,
        "wal": {"path":"accepted-wal", "state_sha256":wal_hash,"sequence":wal_sequence},
        "checkpoint": {"path":"accepted-checkpoint", "state_sha256":checkpoint_hash,"sequence":checkpoint_sequence},
        "new_candidate_compatibility": "not_run",
        "old_historical_world": false
    })).unwrap()).unwrap();
}
'''


def prepare():
    work = (REPO / "work").resolve(strict=True)
    if work != Path("/Volumes/HP P900/dev/MyCity/work"):
        raise RuntimeError("work must resolve to the existing external SSD directory")
    if OUTPUT.exists():
        raise RuntimeError(f"refuse to replace existing source or evidence: {OUTPUT}")
    manifest_bytes = (OLD_EVIDENCE / "freeze.json").read_bytes()
    if hashlib.sha256(manifest_bytes).hexdigest() != OLD_MANIFEST_SHA256:
        raise RuntimeError("fixed v119 source manifest identity mismatch")
    archive_digest = hashlib.sha256()
    with (OLD_EVIDENCE / "source.zip").open("rb") as stream:
        for block in iter(lambda: stream.read(1024 * 1024), b""):
            archive_digest.update(block)
    if archive_digest.hexdigest() != OLD_ARCHIVE_SHA256:
        raise RuntimeError("fixed v119 source archive identity mismatch")
    freeze = json.loads(manifest_bytes)
    if freeze["source_sha256"] != OLD_BUSINESS_SHA256:
        raise RuntimeError("fixed v119 business identity mismatch")
    with zipfile.ZipFile(OLD_EVIDENCE / "source.zip") as archive:
        entries = {entry["path"]: entry["sha256"] for entry in freeze["files"]}
        if set(archive.namelist()) != set(entries):
            raise RuntimeError("old archive file set differs from its fixed manifest")
        for name, expected in entries.items():
            path = PurePosixPath(name)
            if path.is_absolute() or ".." in path.parts:
                raise RuntimeError("archive path traversal")
            if hashlib.sha256(archive.read(name)).hexdigest() != expected:
                raise RuntimeError(f"old source mismatch: {name}")
        OUTPUT.mkdir()
        for name in entries:
            destination = OUTPUT / name
            destination.parent.mkdir(parents=True, exist_ok=True)
            destination.write_bytes(archive.read(name))
    dependencies = []
    for name in FIXTURE_PATHS:
        data = subprocess.check_output(["git", "show", f"{OLD_COMMIT}:{name}"], cwd=REPO)
        destination = OUTPUT / name
        destination.parent.mkdir(parents=True, exist_ok=True)
        destination.write_bytes(data)
        dependencies.append({"path": name, "sha256": hashlib.sha256(data).hexdigest()})
    harness = OUTPUT / "crates/city-storage/tests/legacy_accepted_fixture.rs"
    harness.write_text(HARNESS)
    result = {
        "status": "prepared_not_run", "old_commit": OLD_COMMIT,
        "source_archive_sha256": OLD_ARCHIVE_SHA256,
        "source_manifest_sha256": OLD_MANIFEST_SHA256,
        "business_source_sha256": freeze["source_sha256"],
        "business_file_count": len(entries), "output": str(OUTPUT.resolve()),
        "fixed_doc_dependencies": dependencies,
        "only_added_harness": str(harness.relative_to(OUTPUT)),
        "harness_sha256": hashlib.sha256(harness.read_bytes()).hexdigest(),
        "target_created": False, "cargo_executed": False, "fixtures_generated": False,
        "future_build_command": ["cargo", "+1.85.1", "test", "--locked", "--offline", "--release",
                                 "-p", "mycity-storage", "--test", "legacy_accepted_fixture", "--no-run"],
        "future_writer_command": [str(HERE / "attempt01/writer-test-executable"),
                                  "export_v119_accepted_project_wal_and_checkpoint", "--exact", "--nocapture"],
        "future_execution_order": "build only, preserve exact executable and harness, execute preserved binary once, retain actual receipt/logs, seal outputs",
        "future_environment": {
            "CARGO_TARGET_DIR": "/Users/frankqin/dev/MyCity/target",
            "MYCITY_LEGACY_PROJECT_OUT": "/Volumes/HP P900/dev/MyCity/work/legacy-accepted-project-119-run01",
        },
        "storage": "external HP P900 USB/APFS SSD",
        "never_run_during_formal_measurement": True,
    }
    assert not (OUTPUT / "target").exists()
    (HERE / "preparation.json").write_text(json.dumps(result, indent=2) + "\n")
    print(json.dumps(result, indent=2))


if __name__ == "__main__":
    prepare()
