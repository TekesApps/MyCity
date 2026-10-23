"""Pure frozen-input checks plus read-only HP P900 capacity verification."""
import hashlib
import json
import os
import plistlib
import struct
import subprocess
import zipfile
from pathlib import Path

EXPECTED_SOURCE = "a6290a562362b7ea21379726f901a61cf54d32e8d9fe2059bccf3fe44449d3d8"
EXPECTED_ARCHIVE = "f083a1fe9521f390c16d4665c61ca4f6b2c5461cdf92d7dea52d5ff238391094"
EXPECTED_BINARY = "8b27415ce6dc6a07660b31dd1a31bdb549615347a32bb3675b6582bad57acd26"
EXPECTED_LOCK = "7da5c54a8499a07cbc71dcecce0c0095d4af3b8b6e5052783f20d188037b6dd6"
EXPECTED_ACCEPTANCE = "876ea995ea7834bfcea59b2ef8fa56d502aed9006477bcc67bcaa3f5cd944508"
EXPECTED_OPENING = "04c113c62b0b682f94f9abfbde7618b87bd972f3931504366be9e07c130b4726"
FIXTURES = (
    "fixtures/m0-t13a-acceptance.json",
    "fixtures/m0-t13a-full-opening-candidate.json",
)
EXPECTED_ENTRY_COUNT = 264
EXPECTED_ARCHIVE_ENTRY_COUNT = 276
EXPECTED_CONTEXT_COMMIT = "61587dcc0297924d014ce3c6eb7e47fd6a6503dc"
EXPECTED_CONTEXT_TAG = "v0.5.132"
EXPECTED_SOURCE_MANIFEST = "f81c917add50a6c7a5f65aaec6e6bdded5b808f86890660de88cd16ed8d62b64"
SNAPSHOT_ESTIMATE_BYTES = 3_378_784_402
LOG_CAP_BYTES = 256 * 1024**2
WORLD_CAP_BYTES = 20 * 1024**3
FREE_FLOOR_BYTES = 2 * 1024**3
THREE_GENERATION_ESTIMATE_BYTES = 3 * SNAPSHOT_ESTIMATE_BYTES + LOG_CAP_BYTES
REQUIRED_AVAILABLE_BYTES = THREE_GENERATION_ESTIMATE_BYTES + FREE_FLOOR_BYTES
HP_MOUNT = Path("/Volumes/HP P900")
HP_UUID = "4289CF6D-F09D-4831-8558-4AB223E4EE04"
SUN_PATH_BYTES = 104


def file_sha256(path):
    digest = hashlib.sha256()
    with Path(path).open("rb") as stream:
        for block in iter(lambda: stream.read(1024 * 1024), b""):
            digest.update(block)
    return digest.hexdigest()


def stream_source_sha256(entries):
    """Original sorted-name, little-endian-u64 length-prefixed source hash."""
    digest = hashlib.sha256()
    for name, data in sorted(entries, key=lambda item: item[0]):
        encoded = name.encode()
        digest.update(struct.pack("<Q", len(encoded)))
        digest.update(encoded)
        digest.update(struct.pack("<Q", len(data)))
        digest.update(data)
    return digest.hexdigest()


def verify_archive_bytes_at_commit(entries, repo, commit):
    resolved = subprocess.check_output(["git", "-C", str(repo), "rev-parse", commit], text=True).strip()
    assert resolved == EXPECTED_CONTEXT_COMMIT, f"unexpected prepared commit: {resolved}"
    tag_commit = subprocess.check_output(["git", "-C", str(repo), "rev-parse", EXPECTED_CONTEXT_TAG + "^{commit}"], text=True).strip()
    assert tag_commit == EXPECTED_CONTEXT_COMMIT, f"prepared tag does not point to expected commit: {tag_commit}"
    rows = []
    for name, archive_bytes in sorted(entries, key=lambda item: item[0]):
        completed = subprocess.run(
            ["git", "-C", str(repo), "show", f"{resolved}:{name}"],
            stdout=subprocess.PIPE, stderr=subprocess.PIPE, check=False,
        )
        assert completed.returncode == 0, f"archive path absent from prepared commit: {name}"
        commit_bytes = completed.stdout
        archive_sha = hashlib.sha256(archive_bytes).hexdigest()
        commit_sha = hashlib.sha256(commit_bytes).hexdigest()
        assert commit_bytes == archive_bytes, f"archive bytes differ from {EXPECTED_CONTEXT_TAG}: {name}"
        rows.append({"path": name, "size": len(archive_bytes), "archive_sha256": archive_sha, "v132_git_blob_bytes_sha256": commit_sha, "byte_equal": True})
    return {"status":"passed_all_archive_inputs_byte_equal_to_v132_git_blobs", "tag":EXPECTED_CONTEXT_TAG, "commit":resolved, "count":len(rows), "mismatches":0, "inputs":rows}


def verify_joint_contract(archive_path):
    with zipfile.ZipFile(archive_path) as archive:
        main = archive.read("crates/city-headless/src/main.rs").decode()
        joint = archive.read("crates/city-headless/src/joint_driver.rs").decode()
        pool = archive.read("crates/city-headless/src/normal_pool.rs").decode()
        diagnostic = archive.read("crates/city-headless/src/diagnostic_contract.rs").decode()
        acceptance = json.loads(archive.read("fixtures/m0-t13a-acceptance.json"))
    checks = {
        "entrypoint": 'usage: mycity-headless joint-endurance NEW_DIR smoke|formal' in main,
        "full_business_hours": "completed_hour_targets != 720" in joint,
        "full_drive_targets": "verified_drive_targets != 723" in joint,
        "full_query_count": "queries_completed != 2880" in joint,
        "full_save_count": "saves_completed != 96" in joint,
        "pool_725_grants_returns_acks": all(
            token in joint for token in ("balance.grants != 725", "balance.returns != 725", "balance.fill_acknowledgements != 725")
        ),
        "smoke_600_and_formal_28800_layout": all(
            token in joint for token in ("if formal { 28800 } else { 600 }", "if formal {28800} else {600}")
        ),
        "formal_clock_28800_smoke_clock_72": "if formal { 28800.0 } else { 72.0 }" in joint,
        "full_layout_publications_and_restore": all(
            token in joint for token in ("all_published_scans_completed", "record_restore_passed")
        ),
        "original_64_mib_whole_role_pool": "pub const BYTES: usize = 64 * MIB;" in pool,
        "original_8_mib_shared_diagnostic_boundary": "assert_eq!(super::caps().total, 8 * super::MIB);" in diagnostic,
        "original_acceptance_endurance_events": (
            acceptance["memory"]["diagnostic_hot_bytes"] == 8 * 1024 * 1024
            and acceptance["endurance"]["minimum_wall_seconds"] == 28800
            and acceptance["endurance"]["game_hour_wall_seconds"] == 40
            and acceptance["endurance"]["autosave_requests"] == 96
            and acceptance["endurance"]["worker_cells_kill_not_before_wall_seconds"] == 7200
            and acceptance["endurance"]["sim_committed_reply_kill_not_before_wall_seconds"] == 14400
            and acceptance["endurance"]["normal_cold_reopen_not_before_wall_seconds"] == 21600
            and acceptance["endurance"]["last_business_boundary_must_span_run"] is True
        ),
    }
    if not all(checks.values()):
        raise AssertionError(f"frozen joint-endurance contract mismatch: {checks}")
    return checks


def unix_path_bytes(path):
    """macOS sockaddr_un pathname, including its terminating NUL, in UTF-8 bytes."""
    raw = os.fspath(path).encode("utf-8")
    return len(raw) + 1


def validate_sun_path(path):
    size = unix_path_bytes(path)
    if size >= SUN_PATH_BYTES:
        raise AssertionError(f"Unix socket path including NUL is {size} bytes; must be < {SUN_PATH_BYTES}: {path}")
    return {"path": os.fspath(path), "utf8_bytes_plus_nul": size, "sun_path_bytes": SUN_PATH_BYTES, "valid": True}


def verify_socket_source_inventory(archive_path, work_base, run_names):
    """Find every pathname UDS bind/connect site in the exact frozen Rust source."""
    with zipfile.ZipFile(archive_path) as archive:
        rust_names = [name for name in archive.namelist() if name.endswith(".rs")]
        sources = {name: archive.read(name).decode("utf-8", "replace") for name in rust_names}
    binds = [
        {"source": name, "line": i + 1, "text": line.strip()}
        for name, text in sources.items()
        for i, line in enumerate(text.splitlines())
        if "UnixListener::bind" in line
    ]
    connects = [
        {"source": name, "line": i + 1, "text": line.strip()}
        for name, text in sources.items()
        for i, line in enumerate(text.splitlines())
        if "UnixStream::connect" in line
    ]
    expected_binds = {
        "crates/city-headless/src/normal_pool.rs",
        "crates/city-headless/src/normal_pool/broker.rs",
    }
    expected_connects = expected_binds | {"crates/city-headless/tests/normal_pool.rs"}
    assert {row["source"] for row in binds} == expected_binds, f"unexpected pathname UDS bind sites: {binds}"
    assert {row["source"] for row in connects} == expected_connects, f"unexpected pathname UDS connect sites: {connects}"
    assert all('root.join("pool.sock")' in row["text"] for row in connects if row["source"].endswith("tests/normal_pool.rs"))
    assert 'UnixListener::bind(root.join("pool.sock"))' in sources["crates/city-headless/src/normal_pool.rs"]
    assert 'dir.join("normal-pool")' in sources["crates/city-headless/src/joint_driver.rs"]
    broker = sources["crates/city-headless/src/normal_pool/broker.rs"]
    assert '"/tmp/mc-w-{:x}-{}-{}.sock"' in broker
    assert "ready.boot as u64" in broker and "generation: u64" in broker

    base = Path(work_base)
    resolved_base = base.resolve(strict=True)
    roots = []
    for run_name in run_names:
        for label, work in (("invocation_symlink_path", base), ("resolved_external_path", resolved_base)):
            model_root = work / run_name / "observed/model"
            pool_socket = model_root / "normal-pool/pool.sock"
            roots.append({
                "run_name": run_name,
                "path_basis": label,
                "model_root": str(model_root),
                "normal_pool_socket": validate_sun_path(pool_socket),
            })

    # Rust formats boot as at most 16 hex digits, generation as u64 decimal,
    # and the SHA-256 key prefix as exactly 16 hex digits.
    max_broker_path = "/tmp/mc-w-" + format(2**64 - 1, "x") + "-" + str(2**64 - 1) + "-" + ("f" * 16) + ".sock"
    broker_socket = validate_sun_path(max_broker_path)
    return {
        "status": "passed_frozen_pathname_uds_inventory",
        "path_basis_includes_resolved_external_work": str(resolved_base),
        "sun_path_limit_bytes": SUN_PATH_BYTES,
        "rule": "UTF-8 pathname byte count plus terminating NUL must be strictly less than 104",
        "pathname_bind_sites": binds,
        "pathname_connect_sites": connects,
        "socketpair_transport": "compute-worker uses inherited anonymous UnixStream descriptors; no filesystem pathname",
        "normal_pool_path_derivation": "joint_driver dir.join(normal-pool) -> normal_pool root.join(pool.sock); shared by business/layout workers",
        "run_paths": roots,
        "max_formatted_broker_socket": broker_socket,
        "all_bind_and_connect_paths_fit": True,
    }


def verify_freeze(source, freeze, archive_path, binary_path, canonical_root):
    descriptor_path = Path(source["descriptor_path"])
    assert file_sha256(descriptor_path) == source["descriptor_sha256"] == freeze["descriptor_sha256"]
    descriptor = json.loads(descriptor_path.read_text())
    assert descriptor["source_sha256"] == source["fixed_source_sha256"]
    assert descriptor["source_zip_sha256"] == source["fixed_source_archive_sha256"]
    assert descriptor["binary_sha256"] == source["fixed_binary_sha256"]
    assert Path(descriptor["source_zip"]).resolve(strict=True) == Path(archive_path).resolve(strict=True)
    assert Path(descriptor["binary"]).resolve(strict=True) == Path(binary_path).resolve(strict=True)
    assert descriptor["source_files"] == source["fixed_source_files"] == EXPECTED_ENTRY_COUNT
    assert descriptor["fixed_input_files"] == source["fixed_input_files"] == EXPECTED_ARCHIVE_ENTRY_COUNT
    assert source["prepared_repo_commit"] == freeze["prepared_repo_commit"] == EXPECTED_CONTEXT_COMMIT
    assert source["prepared_repo_tag"] == freeze["prepared_repo_tag"] == EXPECTED_CONTEXT_TAG
    assert source["fixed_source_sha256"] == freeze["source_sha256"] == EXPECTED_SOURCE
    assert source["fixed_source_archive_sha256"] == freeze["source_zip_sha256"] == EXPECTED_ARCHIVE
    assert source["fixed_binary_sha256"] == EXPECTED_BINARY
    assert source["fixed_lock_sha256"] == freeze["lock_sha256"] == EXPECTED_LOCK
    assert source["acceptance_sha256"] == freeze["acceptance_sha256"] == EXPECTED_ACCEPTANCE
    assert source["full_opening_sha256"] == freeze["full_opening_sha256"] == EXPECTED_OPENING
    assert file_sha256(archive_path) == EXPECTED_ARCHIVE

    with zipfile.ZipFile(archive_path) as archive:
        assert archive.testzip() is None, "frozen source archive CRC failure"
        names = archive.namelist()
        assert len(names) == len(set(names)), "duplicate frozen source paths"
        assert len(names) == freeze["fixed_input_files"] == EXPECTED_ARCHIVE_ENTRY_COUNT
        entries = [(name, archive.read(name)) for name in names]
        frozen = dict(entries)
    source_entries = [(name, data) for name, data in entries
                      if name in {".cargo/config.toml", "Cargo.toml", "Cargo.lock", "rust-toolchain.toml"}
                      or name.startswith("crates/") or name.startswith("fixtures/")]
    assert len(source_entries) == freeze["source_files"] == EXPECTED_ENTRY_COUNT
    assert stream_source_sha256(source_entries) == EXPECTED_SOURCE
    assert hashlib.sha256(frozen["Cargo.lock"]).hexdigest() == EXPECTED_LOCK
    manifest_path = Path(source["fixed_source_manifest"])
    assert file_sha256(manifest_path) == EXPECTED_SOURCE_MANIFEST == freeze["source_manifest_sha256"]
    manifest_rows = json.loads(manifest_path.read_text())
    manifest_map = {row["path"]: row for row in manifest_rows}
    assert len(manifest_rows) == EXPECTED_ARCHIVE_ENTRY_COUNT == len(frozen)
    assert set(manifest_map) == set(frozen), "fixed source manifest path set differs from archive"
    for name, data in entries:
        row = manifest_map[name]
        assert row["size"] == len(data), f"fixed source manifest size mismatch: {name}"
        assert row["sha256"] == hashlib.sha256(data).hexdigest(), f"fixed source manifest hash mismatch: {name}"
    archive_to_commit = verify_archive_bytes_at_commit(entries, canonical_root, source["prepared_repo_commit"])

    fixture_hashes = {}
    for fixture, expected in zip(FIXTURES, (EXPECTED_ACCEPTANCE, EXPECTED_OPENING)):
        current = (Path(canonical_root) / fixture).read_bytes()
        assert current == frozen[fixture], f"canonical fixture differs from frozen bytes: {fixture}"
        fixture_hashes[fixture] = hashlib.sha256(current).hexdigest()
        assert fixture_hashes[fixture] == expected

    binary = Path(binary_path)
    assert file_sha256(binary) == EXPECTED_BINARY
    assert EXPECTED_SOURCE.encode() in binary.read_bytes(), "fixed executable lacks source fingerprint"
    joint_checks = verify_joint_contract(archive_path)
    return {
        "status": "passed_static_frozen_input_preflight",
        "source_sha256": EXPECTED_SOURCE,
        "source_entries": len(source_entries),
        "archive_entries": len(names),
        "prepared_repo_tag": EXPECTED_CONTEXT_TAG,
        "prepared_repo_commit": EXPECTED_CONTEXT_COMMIT,
        "archive_to_v132_byte_comparison": archive_to_commit,
        "source_zip_sha256": EXPECTED_ARCHIVE,
        "lock_sha256": EXPECTED_LOCK,
        "fixture_sha256": fixture_hashes,
        "binary_path": str(binary),
        "binary_sha256": EXPECTED_BINARY,
        "binary_embeds_source_fingerprint": True,
        "joint_endurance_contract": joint_checks,
        "current_checkout_comparison": "fixtures_only; current Rust source intentionally not compared byte-for-byte with fixed archive",
        "cargo_invoked": False,
        "pressure_started": False,
    }


def volume_identity(mount):
    output = subprocess.check_output(["/usr/sbin/diskutil", "info", "-plist", str(mount)])
    info = plistlib.loads(output)
    return {
        "mount_point": info.get("MountPoint"),
        "volume_uuid": info.get("VolumeUUID"),
        "filesystem": info.get("FilesystemType", info.get("FilesystemName")),
        "protocol": info.get("DeviceProtocol"),
    }


def verify_storage(work_root):
    resolved = Path(work_root).resolve(strict=True)
    expected_root = (HP_MOUNT / "dev/MyCity/work").resolve(strict=True)
    assert resolved == expected_root or expected_root in resolved.parents, f"work is not on expected HP P900 tree: {resolved}"
    identity = volume_identity(HP_MOUNT)
    assert identity["volume_uuid"] == HP_UUID, f"unexpected HP P900 volume identity: {identity}"
    stat = os.statvfs(resolved)
    available = stat.f_bavail * stat.f_frsize
    assert THREE_GENERATION_ESTIMATE_BYTES < WORLD_CAP_BYTES
    assert available >= REQUIRED_AVAILABLE_BYTES, (
        f"insufficient resolved work-volume free space: {available} < {REQUIRED_AVAILABLE_BYTES}"
    )
    return {
        "resolved_work_root": str(resolved),
        "storage_profile": "external HP P900 USB/APFS SSD; resolved work path and volume UUID verified",
        "volume": identity,
        "available_bytes": available,
        "estimated_snapshot_bytes_each": SNAPSHOT_ESTIMATE_BYTES,
        "estimated_three_generations_plus_logs_bytes": THREE_GENERATION_ESTIMATE_BYTES,
        "combined_log_cap_bytes": LOG_CAP_BYTES,
        "world_cap_bytes": WORLD_CAP_BYTES,
        "required_free_floor_bytes": FREE_FLOOR_BYTES,
        "capacity_gate": "passed_estimate_only; live observer enforces actual 20GiB world and 256MiB combined-log caps",
    }


def verify_retry_directory(directory, canonical_root):
    directory = Path(directory)
    source = json.loads((directory / "source.json").read_text())
    freeze = json.loads((directory / "freeze.json").read_text())
    archive = Path(source["fixed_source_archive"])
    return verify_freeze(source, freeze, archive, source["fixed_binary"], canonical_root)


def verify_support(directory):
    directory = Path(directory)
    manifest = json.loads((directory / "support-manifest.json").read_text())
    source = json.loads((directory / "source.json").read_text())
    assert manifest["fixed_source_sha256"] == source["fixed_source_sha256"]
    assert manifest["fixed_binary_sha256"] == source["fixed_binary_sha256"]
    declared = manifest["files"]
    actual = {name: file_sha256(directory / name) for name in declared}
    assert actual == declared, "fixed runner/support bytes changed"
    return {"status": "passed_fixed_support_hashes", "files": len(actual), "files_sha256": actual}


if __name__ == "__main__":
    here = Path(__file__).resolve().parent
    source = json.loads((here / "source.json").read_text())
    repo = Path(source["prepared_from_repo"]).resolve(strict=True)
    archive = Path(source["fixed_source_archive"])
    freeze = json.loads((here / "freeze.json").read_text())
    result = verify_freeze(source, freeze, archive, source["fixed_binary"], repo)
    result["support"] = verify_support(here)
    result["storage"] = verify_storage(repo / "work")
    paths = source["run_prefixes"]["work_paths"]
    result["socket_paths"] = verify_socket_source_inventory(archive, repo / "work", tuple(paths))
    (here / "unix-socket-paths.json").write_text(json.dumps(result["socket_paths"], indent=2) + "\n")
    result["protected_original_inventory"] = str(repo / source["retention"]["protected_original_inventory"])
    print(json.dumps(result, indent=2))
