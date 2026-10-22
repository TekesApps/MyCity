#!/usr/bin/env python3
"""One-shot, unpaced full-tier timing diagnostic; no performance acceptance."""
import datetime
import hashlib
import importlib.util
import json
import os
import re
import signal
import subprocess
import sys
import time
import zipfile
from pathlib import Path

sys.dont_write_bytecode = True

REPO = Path("/Users/frankqin/dev/MyCity")
WORK = Path("/Volumes/HP P900/dev/MyCity/work")
ROOT = WORK / "timing-full-v128-02"
BINARY_RECORD = WORK / "construction-utilities-current-01/diagnostic-final-binary.json"
HELPER = WORK / "speed-full-v125-01/run_speed.py"
CONTROL = REPO / "docs/evidence/m0-t13a-joint-smoke-control-03"
EXPECTED_HELPER_HASHES = {
    str(HELPER): "32311cb97680b55f58ae4f74f50ead3ab11106421a4e28d706cc9c927ddfb45d",
    str(CONTROL / "readiness.py"): "fa62f21677bab98c558e86fa3a54f42ecfa8b64fd1d7d3449b6401ecff4ef7ae",
    str(CONTROL / "process_activity.py"): "1abbc0771180936fa660b34db4f6428f199fd467e0f866548e8a12bdb4c58aa9",
    str(CONTROL / "support-manifest.json"): "aded3e653c0f0da488ffda0a5110441d8dfc1a97987f1d9fdf89642e5adbebe7",
}
ACCEPTANCE_SHA = "876ea995ea7834bfcea59b2ef8fa56d502aed9006477bcc67bcaa3f5cd944508"
SUN_PATH_BYTES = 104
CHILD_TIMEOUT_SECONDS = 3000
CONTROLLER_DEADLINE_SECONDS = 3200
ADMISSION_SECONDS = 120


def load_helper():
    for filename, expected in EXPECTED_HELPER_HASHES.items():
        if sha256_file(filename) != expected:
            raise RuntimeError(f"pinned helper hash mismatch: {filename}")
    spec = importlib.util.spec_from_file_location("speed_profile_helpers", HELPER)
    module = importlib.util.module_from_spec(spec)
    spec.loader.exec_module(module)
    return module


def sha256_file(path):
    digest = hashlib.sha256()
    with Path(path).open("rb") as stream:
        for block in iter(lambda: stream.read(1024 * 1024), b""):
            digest.update(block)
    return digest.hexdigest()


def write_json(path, value, exclusive=False):
    mode = "x" if exclusive else "w"
    with Path(path).open(mode, encoding="utf-8") as stream:
        json.dump(value, stream, indent=2, sort_keys=True)
        stream.write("\n")


def source_stream_sha256(entries):
    import struct
    digest = hashlib.sha256()
    for name, data in sorted(entries, key=lambda row: row[0]):
        encoded = name.encode("utf-8")
        digest.update(struct.pack("<Q", len(encoded)))
        digest.update(encoded)
        digest.update(struct.pack("<Q", len(data)))
        digest.update(data)
    return digest.hexdigest()


def verify_diagnostic_record(record_path=BINARY_RECORD):
    record = json.loads(Path(record_path).read_text())
    required = ("source_sha256", "binary", "binary_sha256", "source_zip", "source_zip_sha256")
    if any(not isinstance(record.get(key), str) or not record[key] for key in required):
        raise RuntimeError("diagnostic binary record lacks required identity fields")
    for key in ("source_sha256", "binary_sha256", "source_zip_sha256"):
        if not re.fullmatch(r"[0-9a-f]{64}", record[key]):
            raise RuntimeError(f"invalid SHA-256 field: {key}")
    binary, archive_path = Path(record["binary"]), Path(record["source_zip"])
    if sha256_file(binary) != record["binary_sha256"]:
        raise RuntimeError("diagnostic executable digest mismatch")
    if sha256_file(archive_path) != record["source_zip_sha256"]:
        raise RuntimeError("diagnostic source archive digest mismatch")
    with zipfile.ZipFile(archive_path) as archive:
        if archive.testzip() is not None:
            raise RuntimeError("diagnostic source archive CRC failure")
        names = archive.namelist()
        if len(names) != len(set(names)):
            raise RuntimeError("duplicate paths in diagnostic source archive")
        all_entries = [(name, archive.read(name)) for name in names]
    frozen = dict(all_entries)
    build_entries = [(name, data) for name, data in all_entries
                     if name in {".cargo/config.toml", "Cargo.toml", "Cargo.lock", "rust-toolchain.toml"}
                     or name.startswith("crates/") or name.startswith("fixtures/")]
    derived_source = source_stream_sha256(build_entries)
    if derived_source != record["source_sha256"]:
        raise RuntimeError("source archive build-input fingerprint mismatch")
    if record["source_sha256"].encode("ascii") not in binary.read_bytes():
        raise RuntimeError("diagnostic executable does not embed source fingerprint")
    fixture = frozen.get("fixtures/m0-t13a-acceptance.json")
    if fixture is None or hashlib.sha256(fixture).hexdigest() != ACCEPTANCE_SHA:
        raise RuntimeError("acceptance fixture is missing or differs from pinned M0 fixture")
    rust = "\n".join(data.decode("utf-8", "replace") for name, data in all_entries
                     if name.endswith(".rs"))
    workload_checks = {
        "speed-run entrypoint": "speed-run" in rust,
        "unpaced mode": "unpaced" in rust,
        "30 days": '"game_days":30' in rust,
        "720 hours": '"hours":720' in rust,
        "723 targets": '"targets":723' in rust,
        "724 windows": "worker_windows: 724" in rust or "windows != 724" in rust,
    }
    if not all(workload_checks.values()):
        raise RuntimeError("full unpaced workload source checks failed: " + json.dumps(workload_checks))
    return {"source_sha256": derived_source, "binary_sha256": record["binary_sha256"],
            "source_zip_sha256": record["source_zip_sha256"],
            "binary": str(binary.resolve()), "source_zip": str(archive_path.resolve()),
            "binary_embeds_source_fingerprint": True,
            "acceptance_fixture_sha256": ACCEPTANCE_SHA,
            "archive_entries": len(names), "build_source_entries": len(build_entries),
            "full_unpaced_workload_checks": workload_checks}


def validate_clock(clock_map, observer_child, source_sha):
    identity = observer_child.get("identity", {})
    nonce = clock_map.get("nonce", "")
    sent, received = clock_map.get("request_sent_observer_ns"), clock_map.get("reply_received_observer_ns")
    low, high = clock_map.get("supervisor_origin_observer_ns_min"), clock_map.get("supervisor_origin_observer_ns_max")
    return (clock_map.get("source_sha256") == source_sha
            and clock_map.get("supervisor_pid") == identity.get("pid")
            and bool(re.fullmatch(r"[0-9a-f]{32}", nonce))
            and isinstance(sent, int) and isinstance(received, int) and sent <= received
            and isinstance(low, int) and isinstance(high, int) and low <= high
            and clock_map.get("uncertainty_ns") == high - low)


def safe_ps_identity(speed, pid):
    try:
        return speed.ps_identity(pid), None
    except Exception as exc:
        return None, str(exc)


def observer_child_pid(observer_child):
    return ((observer_child or {}).get("identity") or {}).get("pid")


def guarded_readiness_gate(speed, attempt, readiness, preflight):
    """Reuse the pinned gate and always terminate/wait its exact Popen children on exceptions."""
    helpers = []
    popen = speed.subprocess.Popen

    def tracked_popen(*args, **kwargs):
        child = popen(*args, **kwargs)
        helpers.append(child)
        return child

    speed.subprocess.Popen = tracked_popen
    cleanup = []
    try:
        return speed.readiness_gate(attempt, readiness, preflight), cleanup
    finally:
        speed.subprocess.Popen = popen
        for child in helpers:
            if child.poll() is None:
                try:
                    child.terminate()
                    child.wait(timeout=10)
                    cleanup.append({"pid": child.pid, "action": "terminate_own_readiness_helper"})
                except (OSError, subprocess.TimeoutExpired) as exc:
                    cleanup.append({"pid": child.pid, "action": "cleanup_failed", "error": str(exc)})
        readiness["owned_popen_cleanup"] = cleanup


def run():
    # The preparation root exists, but the observer owns observed/ and model/.
    if not ROOT.is_dir():
        raise RuntimeError("prepared timing root is missing")
    for name in ("intent.json", "launch.json", "terminal.json"):
        if (ROOT / name).exists():
            raise RuntimeError(f"one-shot timing root already used: {name}")
    if (ROOT / "observed").exists() or (ROOT / "model").exists():
        raise RuntimeError("refusing to reuse or precreate observer/model path")
    intent = {"schema": "mycity-unpaced-full-timing-v128-v1", "state": "preflight",
              "scope": "full-tier unpaced segment timing diagnostic; no performance/M0/L1/8h/16GB acceptance",
              "started_utc": datetime.datetime.now(datetime.timezone.utc).isoformat(),
              "model_started": False}
    write_json(ROOT / "intent.json", intent, exclusive=True)
    receipt = {**intent, "helper_hashes": EXPECTED_HELPER_HASHES}
    try:
        speed = load_helper()
        identity = verify_diagnostic_record()
        storage = speed.verify_storage(ROOT)
        sockets = speed.socket_preflight("", work_base=ROOT)
        if sockets["normal_pool_socket"]["utf8_bytes_plus_nul"] >= SUN_PATH_BYTES:
            raise RuntimeError("normal-pool socket path exceeds macOS SUN_LEN bound")
        activity = speed.active_activity()
        if activity["compilers"] or activity["heavy_mycity"]:
            raise RuntimeError("active compiler/heavy MyCity process blocks readiness: " + json.dumps(activity))
        receipt.update({"identity": identity, "storage": storage, "socket_paths": sockets,
                        "pre_readiness_processes": activity, "state": "static_preflight_passed"})
    except Exception as exc:
        receipt.update({"state": "blocked_prelaunch", "block_reason": str(exc),
                        "ended_utc": datetime.datetime.now(datetime.timezone.utc).isoformat()})
        write_json(ROOT / "terminal.json", receipt, exclusive=True)
        return 1

    readiness = {}
    try:
        admission, helper_cleanup = guarded_readiness_gate(speed, ROOT, readiness, receipt)
        receipt["readiness_helper_cleanup"] = helper_cleanup
        write_json(ROOT / "readiness-result.json", {"admission": admission, "readiness": readiness}, exclusive=True)
    except Exception as exc:
        receipt.update({"state": "readiness_exception", "error": str(exc), "readiness": readiness,
                        "model_started": False,
                        "ended_utc": datetime.datetime.now(datetime.timezone.utc).isoformat()})
        write_json(ROOT / "terminal.json", receipt, exclusive=True)
        return 1
    if admission.get("status") != "passed":
        receipt.update({"state": "blocked_admission", "admission": admission, "readiness": readiness,
                        "model_started": False,
                        "ended_utc": datetime.datetime.now(datetime.timezone.utc).isoformat()})
        write_json(ROOT / "terminal.json", receipt, exclusive=True)
        return 1

    try:
        if verify_diagnostic_record() != identity:
            raise RuntimeError("binary/source identity changed after readiness")
        if speed.verify_storage(ROOT)["volume"] != storage["volume"]:
            raise RuntimeError("external storage identity changed after readiness")
        if speed.socket_preflight("", work_base=ROOT) != sockets:
            raise RuntimeError("resolved socket paths changed after readiness")
        activity = speed.active_activity()
        if activity["compilers"] or activity["heavy_mycity"]:
            raise RuntimeError("compiler/heavy MyCity process after admission: " + json.dumps(activity))
    except Exception as exc:
        receipt.update({"state": "blocked_post_admission", "block_reason": str(exc),
                        "admission": admission, "model_started": False,
                        "ended_utc": datetime.datetime.now(datetime.timezone.utc).isoformat()})
        write_json(ROOT / "terminal.json", receipt, exclusive=True)
        return 1

    exe = identity["binary"]
    observed, model = ROOT / "observed", ROOT / "observed/model"
    command = [exe, "native-observe", str(observed), "--", exe, "speed-run", str(model), "full", "unpaced"]
    env = {key: value for key, value in os.environ.items() if not key.startswith("MYCITY_")}
    launch = {"state": "launching", "command": command, "source_sha256": identity["source_sha256"],
              "binary_sha256": identity["binary_sha256"], "admission": admission,
              "controller_pid": os.getpid(), "controller_peak": speed.sample_controller_peak(),
              "observer_direct_child": True, "started_utc": datetime.datetime.now(datetime.timezone.utc).isoformat()}
    write_json(ROOT / "launch.json", launch, exclusive=True)
    start = time.monotonic()
    deadline = start + CONTROLLER_DEADLINE_SECONDS
    activity_rows, activity_missing = [], []
    child = None
    code, timed_out, timeout_signal = None, False, "none"
    try:
        with (ROOT / "observer.stdout").open("x") as out, (ROOT / "observer.stderr").open("x") as err:
            child = subprocess.Popen(command, cwd=REPO, stdout=out, stderr=err, env=env)
            child_identity, child_identity_error = safe_ps_identity(speed, child.pid)
            launch.update({"state": "running", "observer_pid": child.pid,
                           "observer_identity": child_identity,
                           "observer_identity_error": child_identity_error})
            write_json(ROOT / "launch.json", launch)
            next_sample = start
            while child.poll() is None:
                now = time.monotonic()
                if now >= deadline:
                    timed_out = True
                    current, identity_error = safe_ps_identity(speed, child.pid)
                    verified = (child.poll() is None and child_identity is not None and current is not None
                                and current["pid"] == child.pid
                                and current["ppid"] == os.getpid() == child_identity["ppid"]
                                and current["start_identity"] == child_identity["start_identity"]
                                and current["comm"] == child_identity["comm"]
                                and Path(current["comm"]).name == Path(exe).name)
                    launch["timeout_identity_recheck"] = {"observed": current, "error": identity_error,
                                                          "verified": verified}
                    if verified:
                        child.terminate()
                        timeout_signal = "SIGTERM_direct_Popen_child_identity_verified"
                        try:
                            code = child.wait(timeout=60)
                        except subprocess.TimeoutExpired:
                            timeout_signal = "SIGTERM_sent_no_exit_within_60s_no_escalation"
                    else:
                        timeout_signal = "withheld_identity_unverified"
                    break
                if now >= next_sample:
                    try:
                        rows = speed.ps_rows()
                        excluded = speed.descendant_pids(rows, child.pid)
                        external = speed.process_activity(rows, excluded | {os.getpid()})
                        if external["compilers"] or external["heavy_mycity"]:
                            activity_rows.append({"elapsed_seconds": now - start, **external})
                    except Exception as exc:
                        activity_missing.append({"elapsed_seconds": now - start, "error": str(exc)})
                    next_sample = now + 5
                time.sleep(min(0.25, max(0.01, deadline - now)))
            else:
                code = child.returncode
    except Exception as exc:
        launch["controller_error"] = str(exc)
        if child is not None and child.poll() is None:
            current, identity_error = safe_ps_identity(speed, child.pid)
            expected = launch.get("observer_identity") or {}
            launch["controller_error_identity_recheck"] = {"observed": current, "error": identity_error}
            verified = (current is not None and expected
                        and current.get("pid") == child.pid
                        and current.get("ppid") == os.getpid() == expected.get("ppid")
                        and current.get("start_identity") == expected.get("start_identity")
                        and current.get("comm") == expected.get("comm"))
            if verified:
                child.terminate()
                try:
                    code = child.wait(timeout=60)
                except subprocess.TimeoutExpired:
                    timeout_signal = "SIGTERM_sent_no_exit_within_60s_no_escalation"
            else:
                timeout_signal = "withheld_identity_unverified_after_controller_error"

    try:
        final_identity = verify_diagnostic_record()
        final_identity_error = None
    except Exception as exc:
        final_identity, final_identity_error = None, str(exc)
    clock_path, child_path = observed / "observer-clock-map.json", observed / "observer-child.json"
    report_path = observed / "observer-report.json"
    clock_valid = report_valid = False
    clock_status, report_status = "missing_or_invalid", "missing_or_invalid"
    try:
        clock = json.loads(clock_path.read_text())
        observer_child = json.loads(child_path.read_text())
        clock_valid = validate_clock(clock, observer_child, identity["source_sha256"])
        clock_status = "passed" if clock_valid else "mismatch"
    except Exception:
        clock, observer_child = {}, {}
    try:
        report = json.loads(report_path.read_text())
        report_valid = (report.get("source_sha256") == identity["source_sha256"]
                        and report.get("binary_sha256") == identity["binary_sha256"]
                        and report.get("binary_unchanged") is True
                        and report.get("status") == "passed_measurement_wrapper"
                        and report.get("model_exit_code") == 0)
        report_status = "passed" if report_valid else "mismatch"
    except Exception:
        report = {}
    valid = (not timed_out and code == 0 and final_identity == identity and clock_valid and report_valid)
    receipt.update({"state": "functional_result_validated_not_performance" if valid else "measurement_failed_or_incomplete",
                    "model_started": child is not None, "admission": admission,
                    "observer_pid": child.pid if child else None, "observer_exit_code": code,
                    "timed_out": timed_out, "timeout_signal": timeout_signal,
                    "child_timeout_seconds": CHILD_TIMEOUT_SECONDS,
                    "controller_deadline_seconds": CONTROLLER_DEADLINE_SECONDS,
                    "elapsed_seconds": time.monotonic() - start,
                    "controller_peak": speed.sample_controller_peak(),
                    "external_compile_or_heavy_activity": activity_rows,
                    "external_activity_sampling_errors": activity_missing,
                    "observer_report_present": report_path.is_file(), "observer_report_status": report_status,
                    "clock_status": clock_status,
                    "clock_supervisor_pid": clock.get("supervisor_pid"),
                    "observer_child_pid": observer_child_pid(observer_child),
                    "source_binary_identity_unchanged": final_identity == identity,
                    "postrun_identity_error": final_identity_error,
                    "scope_statement": "unpaced full functional/segment timing diagnostic only; not clean performance, M0, L1, 8h, or 16GB acceptance",
                    "ended_utc": datetime.datetime.now(datetime.timezone.utc).isoformat()})
    write_json(ROOT / "terminal.json", receipt, exclusive=True)
    return 0 if valid else 1


if __name__ == "__main__":
    try:
        raise SystemExit(run())
    except Exception as exc:
        print(str(exc), file=sys.stderr)
        raise SystemExit(1)
