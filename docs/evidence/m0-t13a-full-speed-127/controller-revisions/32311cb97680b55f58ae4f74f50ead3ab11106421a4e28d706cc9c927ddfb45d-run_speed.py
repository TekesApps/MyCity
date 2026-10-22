#!/usr/bin/env python3
"""Guarded L2 full-speed runner. Never builds, kills unrelated work, or retries."""
import datetime
import hashlib
import json
import os
import plistlib
import re
import resource
import signal
import subprocess
import sys
import time
import zipfile
from pathlib import Path

sys.dont_write_bytecode = True

REPO = Path("/Users/frankqin/dev/MyCity")
EVIDENCE = REPO / "docs/evidence"
CONTROL = EVIDENCE / "m0-t13a-joint-smoke-control-03"
WORK_BASE = Path("/Volumes/HP P900/dev/MyCity/work")
ATTEMPT_BASE = WORK_BASE / "speed-full-v125-01"
POOL_CONTROL = WORK_BASE / "pool-control-current-01"
BINARY_RECORD = POOL_CONTROL / "binary-final.json"
LEGACY_BINARY_RECORD = POOL_CONTROL / "binary.json"
ARCHIVE = POOL_CONTROL / "09-fixed-cli-build/source.zip"
FIXED_BINARY_SHA = "557b2e4d0bc89f429a07a270c9f388b508d595c8b4a1f233b795becdf75797e7"
FIXED_SOURCE_SHA = "ac02e7ccb7e2b9e842e5f8dd15903b14f7fff9b9426909449b36b33bbd08db6c"
FIXED_ARCHIVE_SHA = "8e85f7adc3dd0d0811ec8ee6fa89720512f8ba6f587b1b10de6146cfc3d81561"
FIXED_LOCK_SHA = "7da5c54a8499a07cbc71dcecce0c0095d4af3b8b6e5052783f20d188037b6dd6"
FIXED_ACCEPTANCE_SHA = "876ea995ea7834bfcea59b2ef8fa56d502aed9006477bcc67bcaa3f5cd944508"
LEGACY_SOURCE_SHA_PREFIX = "4a818"
LEGACY_BINARY_SHA = "6279b76ab63cad479b022664c90d68c2a53e373143c4dc72ac7549848e5515ea"
LEGACY_ARCHIVE_SHA = "6f76c28a75a57653020b4f4b3e99ea431d7e70a50f3bf98d1244ce288c528805"
EXPECTED_VOLUME_UUID = "4289CF6D-F09D-4831-8558-4AB223E4EE04"
SNAPSHOT_ESTIMATE_BYTES = 3_378_784_402
LOG_CAP_BYTES = 256 * 1024**2
FREE_FLOOR_BYTES = 2 * 1024**3
REQUIRED_FREE_BYTES = 3 * SNAPSHOT_ESTIMATE_BYTES + LOG_CAP_BYTES + FREE_FLOOR_BYTES
SUN_PATH_BYTES = 104
CHILD_WAIT_SECONDS = 3000
CONTROLLER_DEADLINE_SECONDS = 3200
ADMISSION_SECONDS = 120
ADMISSION_LIMIT_SECONDS = 900


def sha256_file(path):
    digest = hashlib.sha256()
    with Path(path).open("rb") as stream:
        for block in iter(lambda: stream.read(1024 * 1024), b""):
            digest.update(block)
    return digest.hexdigest()


def source_stream_sha256(entries):
    """Sorted-name, little-endian-u64 length-prefixed frozen source hash."""
    import struct
    digest = hashlib.sha256()
    for name, data in sorted(entries, key=lambda row: row[0]):
        encoded = name.encode("utf-8")
        digest.update(struct.pack("<Q", len(encoded)))
        digest.update(encoded)
        digest.update(struct.pack("<Q", len(data)))
        digest.update(data)
    return digest.hexdigest()


def checkout_source_sha256(root):
    """Reproduce build.rs inputs for provenance, without requiring equality."""
    root = Path(root)
    names = {".cargo/config.toml", "Cargo.toml", "Cargo.lock", "rust-toolchain.toml"}
    for base in (root / "crates", root / "fixtures"):
        names.update(p.relative_to(root).as_posix() for p in base.rglob("*") if p.is_file())
    entries = [(name, (root / name).read_bytes()) for name in names]
    return source_stream_sha256(entries), len(entries)


def socket_bytes_plus_nul(path):
    return len(os.fspath(path).encode("utf-8")) + 1


def attempt_name(rate, run_id):
    if rate not in {"1", "10"}:
        raise ValueError("rate must be 1 or 10 seconds per game day")
    allowed = {("1", "speed01"): "rate1-01", ("10", "speed01"): "rate10-01",
               ("10", "speed02"): "rate10-02"}
    if (rate, run_id) not in allowed:
        raise ValueError("unreviewed rate/run identity")
    return allowed[(rate, run_id)]


def validate_fixed_record(record):
    if record.get("source_sha256") != FIXED_SOURCE_SHA:
        raise RuntimeError("binary-final source identity mismatch")
    if record.get("binary_sha256") != FIXED_BINARY_SHA:
        raise RuntimeError("binary-final executable identity mismatch")
    if record.get("source_zip_sha256") != FIXED_ARCHIVE_SHA:
        raise RuntimeError("binary-final source archive identity mismatch")
    return True


def ps_rows():
    # One coherent projection preserves the full executable path, including
    # spaces. There is no cross-snapshot join of unrelated process lifetimes.
    output = subprocess.check_output(
        ["/bin/ps", "-axo", "pid=,ppid=,lstart=,comm="], text=True
    )
    return [{"pid": row["pid"], "ppid": row["ppid"],
             "start_identity": row["start_identity"], "comm": row["value"]}
            for row in parse_ps_listing(output).values()]


def parse_ps_listing(output):
    """Parse fixed PID/PPID/start columns; preserve the entire trailing field."""
    rows = {}
    for line in output.splitlines():
        fields = line.strip().split(None, 7)
        if len(fields) != 8:
            raise RuntimeError("ps row is incomplete or lacks trailing command field")
        try:
            pid, ppid = int(fields[0]), int(fields[1])
        except ValueError as exc:
            raise RuntimeError("ps row has invalid PID/PPID") from exc
        key = (pid, " ".join(fields[2:7]))
        if key in rows:
            raise RuntimeError("duplicate PID/start identity in ps output")
        rows[key] = {"pid": pid, "ppid": ppid, "start_identity": key[1],
                     "value": fields[7]}
    return rows


def process_activity(rows=None, exclude_pids=()):
    excluded = {int(pid) for pid in exclude_pids}
    compilers, heavy = [], []
    for row in rows if rows is not None else ps_rows():
        if row["pid"] in excluded:
            continue
        comm_name = Path(row["comm"]).name
        if comm_name in {"cargo", "rustc", "rustc_driver", "clippy-driver"}:
            compilers.append(row)
        if comm_name in {"mycity-headless", "mycity-storage-worker", "mycity-layout-worker"}:
            heavy.append(row)
    return {"compilers": compilers, "heavy_mycity": heavy}


def descendant_pids(rows, root_pid):
    children = {}
    for row in rows:
        children.setdefault(row["ppid"], []).append(row["pid"])
    found, todo = {int(root_pid)}, [int(root_pid)]
    while todo:
        for pid in children.get(todo.pop(), ()):
            if pid not in found:
                found.add(pid)
                todo.append(pid)
    return found


def volume_identity(mount):
    output = subprocess.check_output(["/usr/sbin/diskutil", "info", "-plist", str(mount)])
    info = plistlib.loads(output)
    return {"mount_point": info.get("MountPoint"), "volume_uuid": info.get("VolumeUUID"),
            "filesystem": info.get("FilesystemType", info.get("FilesystemName")),
            "bus_protocol": info.get("BusProtocol"), "internal": info.get("Internal"),
            "external_device": info.get("RemovableMediaOrExternalDevice")}


def inspect_speed_contract(archive_path):
    with zipfile.ZipFile(archive_path) as archive:
        names = archive.namelist()
        rust = {name: archive.read(name).decode("utf-8", "replace")
                for name in names if name.endswith(".rs")}
        main = rust["crates/city-headless/src/main.rs"]
        speed = rust["crates/city-headless/src/scale_speed.rs"]
        observer = rust["crates/city-headless/src/native_watch/clock.rs"]
        observer_main = rust["crates/city-headless/src/native_watch/mod.rs"]
        observer_system = rust["crates/city-headless/src/native_watch/system.rs"]
    required = {
        "speed_entrypoint": '"speed-run" | "speed-driver"' in main,
        "full_30_days_720_hours_723_targets": all(
            token in speed for token in ('"game_days":30', '"hours":720', '"targets":723')),
        "public_run_selects_full_tier": "run_tier(dir, Tier::Full, rate)" in speed,
        "requested_rate_values": all(token in speed for token in ('"10" => Some(10)', '"1" => Some(1)')),
        "724_worker_windows": "worker_windows: 724" in speed or "windows != 724" in speed,
        "rate_timeout_is_3000_seconds_for_1_and_10": all(token in speed for token in (
            "(u64::from(self.day.unwrap_or(0)) * 30).max(1800) + 1200",
            "if whole.elapsed() > rate.timeout()")),
        "speed_uses_clock_exchange": "crate::observer_clock::synchronize" in speed,
        "native_observer_nonce_clock_peer": all(token in observer for token in (
            "mycity-observer-clock-ready-v1", "mycity-observer-clock-request-v1",
            "mycity-observer-clock-reply-v1", "native_clock_ready_identity",
            "native_clock_reply_identity")),
        "native_observer_keeps_original_guards": all(token in observer_main for token in (
            "usage.total > 20 * 1024 * 1024 * 1024",
            "free < 2 * 1024 * 1024 * 1024",
            "stats.log_bytes + bytes.len() as u64 + usage.latency > 256 * 1024 * 1024",
            "native_measurement_log_limit", "checked_add(250_000_000)")) and all(token in observer_system for token in (
            "maximum_swap_growth > 256 * 1024 * 1024",
            "maximum_non_normal_ns > 60_000_000_000")),
        "normal_pool_path_derivation": '"normal-pool"' in speed and '"pool.sock"' in rust[
            "crates/city-headless/src/normal_pool.rs"],
        "broker_temp_socket_template": '"/tmp/mc-w-{:x}-{}-{}.sock"' in rust[
            "crates/city-headless/src/normal_pool/broker.rs"],
    }
    if not all(required.values()):
        raise RuntimeError("fixed full-speed/clock/socket contract mismatch: " + json.dumps(required))
    bind_sites = [{"source": name, "line": n, "text": line.strip()}
                  for name, text in rust.items() for n, line in enumerate(text.splitlines(), 1)
                  if "UnixListener::bind" in line]
    connect_sites = [{"source": name, "line": n, "text": line.strip()}
                     for name, text in rust.items() for n, line in enumerate(text.splitlines(), 1)
                     if "UnixStream::connect" in line]
    expected_bind = {"crates/city-headless/src/normal_pool.rs",
                     "crates/city-headless/src/normal_pool/broker.rs"}
    if {r["source"] for r in bind_sites} != expected_bind:
        raise RuntimeError("unexpected frozen pathname socket bind sites")
    expected_connect = expected_bind | {"crates/city-headless/tests/normal_pool.rs"}
    if {r["source"] for r in connect_sites} != expected_connect:
        raise RuntimeError("unexpected frozen pathname socket connect sites")
    return {"checks": required, "pathname_binds": bind_sites,
            "pathname_connects": connect_sites,
            "clock_exchange": "nonce-bound startup from native observer; any missing/invalid ready/request/reply is fatal",
            "workload": {"days": 30, "hours": 720, "targets": 723, "worker_windows": 724,
                     "tier": "full", "l1": "absent", "formal_m0": False,
                     "rate_semantics": "1|10 wall seconds per game day; fixed source's timeout resolves to 3000s for both"}}


def verify_fixed_inputs(current_repo=REPO):
    record = json.loads(BINARY_RECORD.read_text())
    validate_fixed_record(record)
    if Path(record["binary"]).resolve() != Path(
            "/Volumes/HP P900/dev/MyCity/work/pool-control-current-01/executables/"
            + FIXED_BINARY_SHA + "/mycity-headless").resolve():
        raise RuntimeError("binary-final.json path does not match immutable expected executable path")
    if record.get("source_zip_sha256") != FIXED_ARCHIVE_SHA or Path(record.get("source_zip", "")) != ARCHIVE:
        raise RuntimeError("binary-final.json source archive identity/path mismatch")
    if sha256_file(ARCHIVE) != FIXED_ARCHIVE_SHA:
        raise RuntimeError("fixed source archive digest mismatch")
    with zipfile.ZipFile(ARCHIVE) as archive:
        if archive.testzip() is not None:
            raise RuntimeError("fixed source archive CRC failure")
        names = archive.namelist()
        if len(names) != len(set(names)):
            raise RuntimeError("duplicate paths in fixed source archive")
        entries = [(name, archive.read(name)) for name in names]
        frozen = dict(entries)
    # Match build.rs exactly: root cargo inputs plus every file in crates/ and
    # fixtures/. The archive also carries five policy documents for provenance;
    # build.rs intentionally excludes those documents from MYCITY_SOURCE_SHA256.
    build_entries = [(name, data) for name, data in entries
                     if name in {".cargo/config.toml", "Cargo.toml", "Cargo.lock", "rust-toolchain.toml"}
                     or name.startswith("crates/") or name.startswith("fixtures/")]
    if len(build_entries) != 259:
        raise RuntimeError(f"fixed source build-input count mismatch: {len(build_entries)} != 259")
    derived_source = source_stream_sha256(build_entries)
    if derived_source != FIXED_SOURCE_SHA:
        raise RuntimeError("fixed source length-prefixed stream hash mismatch")
    if hashlib.sha256(frozen["Cargo.lock"]).hexdigest() != FIXED_LOCK_SHA:
        raise RuntimeError("fixed Cargo.lock digest mismatch")
    if hashlib.sha256(frozen["fixtures/m0-t13a-acceptance.json"]).hexdigest() != FIXED_ACCEPTANCE_SHA:
        raise RuntimeError("fixed acceptance fixture digest mismatch")
    binary = Path(record["binary"])
    if sha256_file(binary) != FIXED_BINARY_SHA:
        raise RuntimeError("fixed executable digest mismatch")
    if FIXED_SOURCE_SHA.encode() not in binary.read_bytes():
        raise RuntimeError("fixed executable does not embed ac02 source fingerprint")
    canonical_fixture = Path(current_repo) / "fixtures/m0-t13a-acceptance.json"
    if canonical_fixture.read_bytes() != frozen["fixtures/m0-t13a-acceptance.json"]:
        raise RuntimeError("canonical acceptance fixture differs from fixed archive")
    current = subprocess.check_output(["git", "-C", str(current_repo), "rev-parse", "HEAD"], text=True).strip()
    current_version = subprocess.check_output(
        ["git", "-C", str(current_repo), "describe", "--tags", "--always", "--dirty"], text=True
    ).strip()
    current_source, current_count = checkout_source_sha256(current_repo)
    legacy = json.loads(LEGACY_BINARY_RECORD.read_text())
    legacy_source = legacy.get("source_sha256", "")
    if (legacy.get("binary_sha256") != LEGACY_BINARY_SHA
            or legacy.get("source_zip_sha256") != LEGACY_ARCHIVE_SHA
            or legacy_source.startswith(FIXED_SOURCE_SHA)):
        raise RuntimeError("legacy binary.json exclusion identity changed or aliases fixed binary-final")
    contract = inspect_speed_contract(ARCHIVE)
    return {"status": "passed_static_fixed_inputs", "source_sha256": derived_source,
            "source_archive_sha256": FIXED_ARCHIVE_SHA, "archive_entries": len(names),
            "build_source_entries": len(build_entries),
            "lock_sha256": FIXED_LOCK_SHA, "acceptance_fixture_sha256": FIXED_ACCEPTANCE_SHA,
            "binary_path": str(binary), "binary_sha256": FIXED_BINARY_SHA,
            "binary_embeds_source_fingerprint": True,
            "prepared_checkout_provenance_only": {"path": str(current_repo), "commit": current,
                                                   "version": current_version,
                                                   "source_sha256": current_source,
                                                   "source_entries": current_count,
                                                   "matches_fixed_source_hash": current_source == FIXED_SOURCE_SHA,
                                                   "source_comparison": "current checkout hash is recorded separately; Rust source intentionally not compared byte-for-byte"},
            "legacy_binary_excluded": {"path": str(LEGACY_BINARY_RECORD),
                                       "binary_sha256": legacy.get("binary_sha256"),
                                       "source_zip_sha256": legacy.get("source_zip_sha256"),
                                       "source_sha256_field": legacy_source or None,
                                       "historical_source_prefix": LEGACY_SOURCE_SHA_PREFIX,
                                       "reason": "binary.json's pinned 6279/6f76 legacy identity is excluded; only binary-final ac02/557b is eligible"},
            "speed_contract": contract}


def verify_storage(path=WORK_BASE):
    mount = Path("/Volumes/HP P900")
    if not mount.is_dir():
        raise RuntimeError("HP P900 mount is absent")
    resolved = Path(path).resolve(strict=True)
    resolved_mount = mount.resolve(strict=True)
    if resolved != resolved_mount and resolved_mount not in resolved.parents:
        raise RuntimeError("resolved work path is not on HP P900")
    identity = volume_identity(mount)
    if identity["volume_uuid"] != EXPECTED_VOLUME_UUID:
        raise RuntimeError("HP P900 UUID mismatch: " + json.dumps(identity))
    if (identity["filesystem"] != "apfs" or identity["bus_protocol"] != "USB"
            or identity["internal"] is not False or identity["external_device"] is not True):
        raise RuntimeError("HP P900 is not the expected external USB/APFS volume: " + json.dumps(identity))
    stat = os.statvfs(resolved)
    free_bytes = stat.f_bavail * stat.f_frsize
    if free_bytes < REQUIRED_FREE_BYTES:
        raise RuntimeError(f"free space {free_bytes} below required {REQUIRED_FREE_BYTES}")
    return {"resolved_work": str(resolved), "volume": identity,
            "available_bytes": free_bytes, "estimated_snapshot_bytes_each": SNAPSHOT_ESTIMATE_BYTES,
            "three_generations_plus_log_cap_bytes": 3 * SNAPSHOT_ESTIMATE_BYTES + LOG_CAP_BYTES,
            "free_floor_bytes": FREE_FLOOR_BYTES, "required_free_bytes": REQUIRED_FREE_BYTES,
            "retention": "keep both full run roots and source/executable until explicit evidence review; never clean automatically"}


def socket_preflight(run_name, work_base=ATTEMPT_BASE):
    resolved = Path(work_base).resolve(strict=True)
    observed = resolved / run_name / "observed"
    model = observed / "model"
    pool = model / "normal-pool" / "pool.sock"
    size = socket_bytes_plus_nul(pool)
    broker = "/tmp/mc-w-" + format(2**64 - 1, "x") + "-" + str(2**64 - 1) + "-" + ("f" * 16) + ".sock"
    broker_size = socket_bytes_plus_nul(broker)
    if size >= SUN_PATH_BYTES or broker_size >= SUN_PATH_BYTES:
        raise RuntimeError("macOS sockaddr_un path must be <104 UTF-8 bytes including NUL")
    return {"normal_pool_socket": {"path": str(pool), "utf8_bytes_plus_nul": size,
                                   "limit_exclusive": SUN_PATH_BYTES},
            "max_formatted_broker_socket": {"path": broker, "utf8_bytes_plus_nul": broker_size,
                                             "limit_exclusive": SUN_PATH_BYTES},
            "source_inventory": "verified from exact source archive; both normal-pool and broker bind/connect families retained"}


def active_activity(exclude_pids=()):
    return process_activity(exclude_pids=(os.getpid(), *exclude_pids))


def verify_observer_support():
    frozen_support = json.loads((CONTROL / "support-manifest.json").read_text())
    used = ("readiness.py", "process_activity.py")
    actual = {file: sha256_file(CONTROL / file) for file in used}
    manifest_sha = sha256_file(CONTROL / "support-manifest.json")
    if manifest_sha != "aded3e653c0f0da488ffda0a5110441d8dfc1a97987f1d9fdf89642e5adbebe7":
        raise RuntimeError("published admission support manifest changed")
    for file in used:
        if actual[file] != frozen_support["files"][file]:
            raise RuntimeError("published admission support file hash changed: " + file)
    actual["support-manifest.json"] = manifest_sha
    actual["usage"] = "readiness.py and process_activity.py only; fixed ac02 native-observe handles measurement and clock handshake"
    return actual


def validate_clock_map(clock_map, observer_child):
    identity = observer_child.get("identity", {})
    nonce = clock_map.get("nonce", "")
    sent = clock_map.get("request_sent_observer_ns")
    received = clock_map.get("reply_received_observer_ns")
    model_origin = clock_map.get("supervisor_origin_observer_ns_min")
    model_origin_max = clock_map.get("supervisor_origin_observer_ns_max")
    return (clock_map.get("source_sha256") == FIXED_SOURCE_SHA
            and clock_map.get("supervisor_pid") == identity.get("pid")
            and bool(re.fullmatch(r"[0-9a-f]{32}", nonce))
            and isinstance(sent, int) and isinstance(received, int) and sent <= received
            and isinstance(model_origin, int) and isinstance(model_origin_max, int)
            and model_origin <= model_origin_max
            and clock_map.get("uncertainty_ns") == model_origin_max - model_origin)


def sample_controller_peak():
    usage = resource.getrusage(resource.RUSAGE_SELF)
    peak = int(usage.ru_maxrss)
    return {"pid": os.getpid(), "ru_maxrss_platform_units": peak,
            "macos_ru_maxrss_bytes": peak, "accounting": "controller-only; observer and its descendants are raw observer group, never added here"}


def read_root_receipt(path, value):
    path.write_text(json.dumps(value, indent=2, sort_keys=True) + "\n")


def ps_identity(pid):
    rows = [row for row in ps_rows() if row["pid"] == pid]
    if len(rows) != 1:
        return None
    return {"pid": pid, "ppid": rows[0]["ppid"], "start_identity": rows[0]["start_identity"],
            "comm": rows[0]["comm"], "projection": "single_ps_full_executable_not_argv"}


def readiness_gate(attempt, readiness, preflight):
    readiness_cmd = [sys.executable, str(CONTROL / "readiness.py"), str(attempt)]
    env = {k: v for k, v in os.environ.items() if not k.startswith("MYCITY_")}
    (attempt / "measurement-requested").touch(exist_ok=False)
    began = time.monotonic()
    activities = []
    with (attempt / "readiness.stdout").open("x") as out, (attempt / "readiness.stderr").open("x") as err:
        child = subprocess.Popen(readiness_cmd, cwd=REPO, stdout=out, stderr=err, env=env)
        readiness["pid"] = child.pid
        while child.poll() is None:
            try:
                activity = active_activity()
                if activity["compilers"] or activity["heavy_mycity"]:
                    activities.append({"elapsed_seconds": time.monotonic() - began, **activity})
            except Exception as exc:
                activities.append({"elapsed_seconds": time.monotonic() - began,
                                   "sampling_error": str(exc)})
            if time.monotonic() - began > ADMISSION_LIMIT_SECONDS + 30:
                # This is our own readiness helper only; it cannot own a model.
                child.terminate()
                child.wait(timeout=10)
                return {"exit_code": child.returncode, "status": "readiness_deadline", "activity": activities}
            time.sleep(5)
        code = child.returncode
    readiness.update({"exit_code": code, "elapsed_seconds": time.monotonic() - began,
                      "activity_observed": activities})
    gate = json.loads((attempt / "readiness.json").read_text()) if (attempt / "readiness.json").exists() else None
    window_start = max(0.0, readiness["elapsed_seconds"] - ADMISSION_SECONDS)
    recent_activity = [row for row in activities if row["elapsed_seconds"] >= window_start]
    passed = (code == 0 and gate is not None and not recent_activity
              and len(gate.get("samples", [])) >= 25
              and gate["samples"][-1]["elapsed_seconds"] - gate["samples"][0]["elapsed_seconds"] >= ADMISSION_SECONDS
              and all(row["pressure"] == 1 and not row["active_external_compilers"] for row in gate["samples"]))
    return {"exit_code": code, "status": "passed" if passed else "blocked",
            "normal_pressure_gate": gate, "activity_observed": activities,
            "final_120_seconds_start": window_start,
            "activity_in_final_120_seconds": recent_activity}


def run_one(rate, run_id):
    name = attempt_name(rate, run_id)
    attempt = ATTEMPT_BASE / name
    ATTEMPT_BASE.mkdir(parents=True, exist_ok=True)
    attempt.mkdir(exist_ok=False)
    observed = attempt / "observed"  # Deliberately not created here; native observer owns it.
    identity = {"rate_seconds_per_game_day": int(rate), "run_id": run_id,
                "attempt_root": str(attempt), "source_sha256": FIXED_SOURCE_SHA,
                "binary_sha256": FIXED_BINARY_SHA,
                "runner_sha256": sha256_file(__file__),
                "scope": "standalone L2 full speed only; L1/joint/four-rate M0/formal resource acceptance not claimed"}
    receipt = {"schema": "mycity-standalone-full-speed-receipt-v1", **identity,
               "state": "blocked_prelaunch", "model_started": False,
               "started_utc": datetime.datetime.now(datetime.timezone.utc).isoformat(),
               "timeout": {"child_wait_seconds": CHILD_WAIT_SECONDS,
                           "controller_deadline_seconds": CONTROLLER_DEADLINE_SECONDS,
                           "restart": "never", "signal_policy": "SIGTERM only exact still-live own observer PID/start/command identity; no escalation"}}
    read_root_receipt(attempt / "intent.json", receipt)
    try:
        if Path(attempt).resolve().exists() is False:
            raise RuntimeError("attempt path resolution failed")
        frozen = verify_fixed_inputs()
        storage = verify_storage()
        sockets = socket_preflight(name)
        activity = active_activity()
        if activity["compilers"] or activity["heavy_mycity"]:
            raise RuntimeError("blocked by active compile/heavy MyCity process: " + json.dumps(activity))
        support = verify_observer_support()
        receipt.update({"frozen_inputs": frozen, "storage": storage, "socket_paths": sockets,
                        "admission_support_files_sha256": support,
                        "active_processes_before_readiness": activity,
                        "state": "static_preflight_passed_readiness_not_run"})
        read_root_receipt(attempt / "preflight.json", receipt)
    except Exception as exc:
        receipt.update({"block_reason": str(exc), "state": "blocked_prelaunch_not_run",
                        "ended_utc": datetime.datetime.now(datetime.timezone.utc).isoformat()})
        read_root_receipt(attempt / "receipt.json", receipt)
        raise

    readiness_info = {}
    admission = readiness_gate(attempt, readiness_info, receipt)
    read_root_receipt(attempt / "admission.json", {"admission": admission, "readiness": readiness_info})
    if admission["status"] != "passed":
        receipt.update({"state": "blocked_admission_not_run", "admission": admission,
                        "model_started": False,
                        "ended_utc": datetime.datetime.now(datetime.timezone.utc).isoformat()})
        read_root_receipt(attempt / "receipt.json", receipt)
        return 1

    # Rebind the fixed inputs and activity immediately after readiness.
    try:
        if frozen != verify_fixed_inputs():
            raise RuntimeError("fixed source/executable identity changed after readiness")
        if storage["volume"] != verify_storage()["volume"]:
            raise RuntimeError("storage volume identity changed after readiness")
        if sockets != socket_preflight(name):
            raise RuntimeError("resolved socket paths changed after readiness")
        if support != verify_observer_support():
            raise RuntimeError("observer support identity changed after readiness")
        activity = active_activity()
        if activity["compilers"] or activity["heavy_mycity"]:
            raise RuntimeError("active compile/heavy MyCity process after readiness: " + json.dumps(activity))
    except Exception as exc:
        receipt.update({"state": "blocked_post_admission_not_run", "block_reason": str(exc),
                        "admission": admission, "model_started": False,
                        "ended_utc": datetime.datetime.now(datetime.timezone.utc).isoformat()})
        read_root_receipt(attempt / "receipt.json", receipt)
        return 1

    exe = str(json.loads(BINARY_RECORD.read_text())["binary"])
    model = str(observed / "model")
    command = [exe, "native-observe", str(observed), "--", exe, "speed-run", model, "full", rate]
    env = {k: v for k, v in os.environ.items() if not k.startswith("MYCITY_")}
    receipt.update({"state": "running", "model_started": True, "admission": admission,
                    "command": command, "observer_pid_role": "direct native-observe child",
                    "controller_pid": os.getpid(), "controller_peak_resource": sample_controller_peak(),
                    "native_observer_owns_descendant_liveness_guard": True,
                    "controller_auxiliary_accounting": "controller-only; observer raw-group RSS/footprint not added twice",
                    "launch_utc": datetime.datetime.now(datetime.timezone.utc).isoformat()})
    read_root_receipt(attempt / "live.json", receipt)
    start = time.monotonic()
    deadline = start + CONTROLLER_DEADLINE_SECONDS
    child_identity = None
    timed_out = False
    max_rss = int(resource.getrusage(resource.RUSAGE_SELF).ru_maxrss)
    activity_rows = []
    activity_missing = []
    with (attempt / "observer.stdout").open("x") as out, (attempt / "observer.stderr").open("x") as err:
        child = subprocess.Popen(command, cwd=REPO, stdout=out, stderr=err, env=env)
        child_identity = ps_identity(child.pid)
        receipt["observer_pid"] = child.pid
        receipt["observer_pid_start_identity"] = child_identity
        read_root_receipt(attempt / "live.json", receipt)
        next_resource_sample = start
        next_activity_sample = start
        while child.poll() is None:
            now = time.monotonic()
            if now >= deadline:
                timed_out = True
                current = ps_identity(child.pid)
                verified = (child.poll() is None and child_identity is not None and current is not None
                            and current["start_identity"] == child_identity["start_identity"]
                            and current["comm"] == child_identity["comm"]
                            and current["ppid"] == child_identity["ppid"] == os.getpid()
                            and Path(current["comm"]).name == Path(exe).name)
                receipt["timeout_identity_recheck"] = {"observed": current, "verified": verified}
                if verified:
                    os.kill(child.pid, signal.SIGTERM)
                    receipt["timeout_signal"] = "SIGTERM_direct_observer_only"
                    read_root_receipt(attempt / "live.json", receipt)
                    try:
                        code = child.wait(timeout=60)
                    except subprocess.TimeoutExpired:
                        code = None
                        receipt["timeout_grace"] = "observer still live after 60s; no escalation, no restart"
                else:
                    code = None
                    receipt["timeout_signal"] = "withheld_identity_unverified"
                break
            if now >= next_resource_sample:
                max_rss = max(max_rss, int(resource.getrusage(resource.RUSAGE_SELF).ru_maxrss))
                next_resource_sample = now + 1
            if now >= next_activity_sample:
                try:
                    process_rows = ps_rows()
                    excluded = descendant_pids(process_rows, child.pid)
                    external = process_activity(process_rows, excluded | {os.getpid()})
                    if external["compilers"] or external["heavy_mycity"]:
                        activity_rows.append({"elapsed_seconds": now - start, **external})
                except Exception as exc:
                    activity_missing.append({"elapsed_seconds": now - start, "error": str(exc)})
                next_activity_sample = now + 5
            time.sleep(min(0.25, max(0.01, deadline - now)))
        else:
            code = child.returncode

    receipt.update({"state": "timeout" if timed_out else "observer_exited",
                    "observer_exit_code": code, "timed_out": timed_out,
                    "elapsed_seconds": time.monotonic() - start,
                    "controller_peak_resource": {"pid": os.getpid(), "ru_maxrss_platform_units": max_rss,
                        "macos_ru_maxrss_bytes": max_rss,
                        "accounting": "controller-only; not added to raw observer group"},
                    "observer_report_path": str(observed / "observer-report.json"),
                    "observer_report_present": (observed / "observer-report.json").is_file(),
                    "external_compile_or_heavy_mycity_observations": activity_rows,
                    "external_activity_sample_missing": activity_missing,
                    "external_activity_accounting": "controller observations outside this observer PID tree; raw observer group remains unchanged",
                    "ended_utc": datetime.datetime.now(datetime.timezone.utc).isoformat(),
                    "performance_acceptance": "not_claimed; standalone L2 speed evidence only"})
    clock_path = observed / "observer-clock-map.json"
    child_path = observed / "observer-child.json"
    observer_report_path = observed / "observer-report.json"
    clock_valid = False
    clock_detail = {"status": "missing_or_unreadable", "path": str(clock_path)}
    report_valid = False
    report_detail = {"status": "missing_or_unreadable", "path": str(observer_report_path)}
    try:
        clock_map = json.loads(clock_path.read_text())
        observer_child = json.loads(child_path.read_text())
        clock_valid = validate_clock_map(clock_map, observer_child)
        clock_detail = {"status": "passed" if clock_valid else "mismatch",
                        "source_sha256": clock_map.get("source_sha256"),
                        "observer_child_pid": observer_child.get("identity", {}).get("pid"),
                        "clock_map_pid": clock_map.get("supervisor_pid"),
                        "nonce": clock_map.get("nonce"),
                        "uncertainty_ns": clock_map.get("uncertainty_ns")}
    except (OSError, ValueError, TypeError):
        pass
    try:
        observer_report = json.loads(observer_report_path.read_text())
        report_valid = (observer_report.get("source_sha256") == FIXED_SOURCE_SHA
                        and observer_report.get("binary_sha256") == FIXED_BINARY_SHA
                        and observer_report.get("binary_unchanged") is True
                        and observer_report.get("status") == "passed_measurement_wrapper"
                        and observer_report.get("model_exit_code") == 0)
        report_detail = {"status": "passed" if report_valid else "mismatch",
                         "observer_status": observer_report.get("status"),
                         "source_sha256": observer_report.get("source_sha256"),
                         "binary_sha256": observer_report.get("binary_sha256"),
                         "binary_unchanged": observer_report.get("binary_unchanged"),
                         "model_exit_code": observer_report.get("model_exit_code")}
    except (OSError, ValueError, TypeError):
        pass
    receipt["clock_handshake"] = clock_detail
    receipt["observer_result_validation"] = report_detail
    if not timed_out and code == 0 and (not clock_valid or not report_valid):
        receipt["state"] = "observer_exit_but_receipt_validation_failed"
    read_root_receipt(attempt / "receipt.json", receipt)
    return 0 if not timed_out and code == 0 and clock_valid and report_valid else 1


def main(argv=None):
    argv = sys.argv[1:] if argv is None else argv
    if len(argv) != 2:
        raise SystemExit("usage: run_speed.py RATE(1|10) RUN_ID(speed01|reviewed speed02)")
    rate, run_id = argv
    try:
        return run_one(rate, run_id)
    except Exception as exc:
        print(str(exc), file=sys.stderr)
        return 1


if __name__ == "__main__":
    raise SystemExit(main())
