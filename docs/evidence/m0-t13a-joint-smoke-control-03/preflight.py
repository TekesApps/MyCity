"""Pure frozen-input checks plus read-only HP P900 capacity verification."""
import hashlib
import json
import os
import plistlib
import struct
import subprocess
import zipfile
from pathlib import Path

EXPECTED_SOURCE = "32c6e61b9565545177c330f34455611b329fbe1cb38591ec5f6e7879731c4da2"
EXPECTED_ARCHIVE = "84c450c2c866578b02205030c7587df1dbb719660422fe64865ae62fe2ce908d"
EXPECTED_BINARY = "59aa365ed46b5d373218fcbcf1d34f930d674d15607ed510190361891439b4e0"
EXPECTED_LOCK = "7da5c54a8499a07cbc71dcecce0c0095d4af3b8b6e5052783f20d188037b6dd6"
EXPECTED_ACCEPTANCE = "876ea995ea7834bfcea59b2ef8fa56d502aed9006477bcc67bcaa3f5cd944508"
EXPECTED_OPENING = "04c113c62b0b682f94f9abfbde7618b87bd972f3931504366be9e07c130b4726"
FIXTURES = (
    "fixtures/m0-t13a-acceptance.json",
    "fixtures/m0-t13a-full-opening-candidate.json",
)
EXPECTED_ENTRY_COUNT = 231
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


def verify_joint_contract(archive_path):
    with zipfile.ZipFile(archive_path) as archive:
        main = archive.read("crates/city-headless/src/main.rs").decode()
        joint = archive.read("crates/city-headless/src/joint_driver.rs").decode()
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
        assert len(names) == freeze["source_files"] == EXPECTED_ENTRY_COUNT
        entries = [(name, archive.read(name)) for name in names]
        frozen = dict(entries)
    assert stream_source_sha256(entries) == EXPECTED_SOURCE
    assert hashlib.sha256(frozen["Cargo.lock"]).hexdigest() == EXPECTED_LOCK

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
        "source_entries": len(names),
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
    archive = directory.parent / "m0-t13a-extents-joint-retry-02/frozen-source.zip"
    return verify_freeze(source, freeze, archive, source["fixed_binary"], canonical_root)


if __name__ == "__main__":
    here = Path(__file__).resolve().parent
    repo = here.parents[2]
    source = json.loads((here / "source.json").read_text())
    archive = here.parent / "m0-t13a-extents-joint-retry-02/frozen-source.zip"
    freeze = json.loads((here / "freeze.json").read_text())
    result = verify_freeze(source, freeze, archive, source["fixed_binary"], repo)
    result["storage"] = verify_storage(repo / "work")
    paths = source["run_prefixes"]["work_paths"]
    result["socket_paths"] = verify_socket_source_inventory(archive, repo / "work", tuple(paths))
    (here / "unix-socket-paths.json").write_text(json.dumps(result["socket_paths"], indent=2) + "\n")
    result["protected_original_inventory"] = str(repo / source["retention"]["protected_original_inventory"])
    print(json.dumps(result, indent=2))
