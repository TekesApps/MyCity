#!/usr/bin/env python3
"""One-shot guarded launch for the v128 combined-pool native diagnostic."""
from __future__ import annotations

import datetime
import hashlib
import importlib.util
import json
import os
from pathlib import Path
import subprocess
import sys
import time

sys.dont_write_bytecode = True

BASE = Path(__file__).resolve().parent
REPO = Path("/Users/frankqin/dev/MyCity")
CONTROLLER = BASE / "fault-cases.py"
BINARY_RECORD = BASE / "binary-v128.json"
PROFILE = BASE / "support/run_profile.py"
FREEZE = BASE / "launch-freeze.json"
MANIFEST = BASE / "prepared-manifest.json"
CASE = "combined-control"
MODE = "combined-control"
ADMISSION_SECONDS = 120
CONTROLLER_DEADLINE_SECONDS = 120
SUN_PATH_BYTES = 104
SUPPORT_PATHS = {
    "/Volumes/HP P900/dev/MyCity/work/timing-full-v128-02/run_profile.py":
        "596b8777817e806fbc3f824114d7c222725df4037140b0d2f9934ef5cd33683a",
    "/Volumes/HP P900/dev/MyCity/work/speed-full-v125-01/run_speed.py":
        "32311cb97680b55f58ae4f74f50ead3ab11106421a4e28d706cc9c927ddfb45d",
    "/Users/frankqin/dev/MyCity/docs/evidence/m0-t13a-joint-smoke-control-03/readiness.py":
        "fa62f21677bab98c558e86fa3a54f42ecfa8b64fd1d7d3449b6401ecff4ef7ae",
    "/Users/frankqin/dev/MyCity/docs/evidence/m0-t13a-joint-smoke-control-03/process_activity.py":
        "1abbc0771180936fa660b34db4f6428f199fd467e0f866548e8a12bdb4c58aa9",
    "/Users/frankqin/dev/MyCity/docs/evidence/m0-t13a-joint-smoke-control-03/support-manifest.json":
        "aded3e653c0f0da488ffda0a5110441d8dfc1a97987f1d9fdf89642e5adbebe7",
    "/Users/frankqin/dev/MyCity/docs/evidence/m0-t13a-typed-workspace/observe.py":
        "6bc89147a64894599d7eea1f6762fbb01f4119d606842ce475ba2c2f7215c98a",
    "/Users/frankqin/dev/MyCity/docs/evidence/m0-t13a-typed-workspace/native_metrics.py":
        "917b184e339649acd7b79797cf77f2b2db4f5128f33fdaf8759c162672fa34ca",
}


def utc_now() -> str:
    return datetime.datetime.now(datetime.timezone.utc).isoformat()


def sha256_file(path: Path | str) -> str:
    digest = hashlib.sha256()
    with Path(path).open("rb") as stream:
        for block in iter(lambda: stream.read(1024 * 1024), b""):
            digest.update(block)
    return digest.hexdigest()


def write_json(path: Path, value: dict, exclusive: bool = False) -> None:
    mode = "x" if exclusive else "w"
    with path.open(mode, encoding="utf-8") as stream:
        json.dump(value, stream, indent=2, sort_keys=True)
        stream.write("\n")
        stream.flush()
        os.fsync(stream.fileno())


def socket_bytes(path: Path) -> int:
    return len(os.fspath(path).encode("utf-8")) + 1


def verify_frozen_files() -> dict:
    manifest = json.loads(MANIFEST.read_text(encoding="utf-8"))
    freeze = json.loads(FREEZE.read_text(encoding="utf-8"))
    if sha256_file(MANIFEST) != freeze.get("prepared_manifest_sha256"):
        raise RuntimeError("prepared manifest differs from launch freeze")
    snapshot_manifest = BASE / "controller-revisions/launch/prepared-manifest.json"
    if not snapshot_manifest.is_file() or sha256_file(snapshot_manifest) != freeze.get("prepared_manifest_sha256"):
        raise RuntimeError("launch snapshot manifest differs from frozen manifest")
    files = manifest.get("files", {})
    if freeze.get("files") != files or freeze.get("external_support") != manifest.get("external_support"):
        raise RuntimeError("launch freeze inventory differs from prepared manifest")
    if not files:
        raise RuntimeError("prepared manifest has no frozen files")
    for relative, expected in files.items():
        path = BASE / relative
        if not path.is_file() or sha256_file(path) != expected:
            raise RuntimeError("frozen local file mismatch: " + relative)
        snapshot = BASE / "controller-revisions/launch" / relative
        if not snapshot.is_file() or sha256_file(snapshot) != expected:
            raise RuntimeError("launch snapshot mismatch: " + relative)
    for path, expected in manifest.get("external_support", {}).items():
        if sha256_file(path) != expected:
            raise RuntimeError("frozen external support mismatch: " + path)
    if sha256_file(PROFILE) != manifest["files"].get("support/run_profile.py"):
        raise RuntimeError("local profile helper identity mismatch")
    return manifest


def load_profile():
    spec = importlib.util.spec_from_file_location("v128_native_profile", PROFILE)
    if spec is None or spec.loader is None:
        raise RuntimeError("cannot load frozen timing profile helper")
    module = importlib.util.module_from_spec(spec)
    spec.loader.exec_module(module)
    return module


def controller_result_ok(code: int | None, receipt: dict | None,
                         source_sha: str, binary_sha: str) -> bool:
    return bool(code == 0 and receipt
                and receipt.get("mode") == MODE
                and receipt.get("passed_controlled_exercise") is True
                and receipt.get("performance_acceptance") == "not_claimed"
                and receipt.get("source_sha256") == source_sha
                and receipt.get("binary_sha256") == binary_sha)


def run() -> int:
    if not BASE.is_dir():
        raise RuntimeError("prepared external work root is missing")
    if any((BASE / name).exists() for name in
           ("intent.json", "launch.json", "terminal.json", "observed", CASE)):
        raise RuntimeError("v128-native-01 already has execution artifacts; refusing reuse")

    intent = {
        "schema": "mycity-v128-native-diagnostic-intent-v1",
        "state": "preflight", "model_started": False,
        "scope": "short combined-pool native peak diagnostic only; not full shared 8MiB acceptance, L1, performance, or M0",
        "mode": MODE, "case_root": str(BASE / CASE),
        "controller_deadline_seconds": CONTROLLER_DEADLINE_SECONDS,
        "admission_seconds": ADMISSION_SECONDS,
        "started_utc": utc_now(),
    }
    write_json(BASE / "intent.json", intent, exclusive=True)
    terminal = {**intent, "local_controller_sha256": sha256_file(Path(__file__)),
                "controller_sha256": sha256_file(CONTROLLER),
                "descriptor_sha256": sha256_file(BINARY_RECORD)}
    child = None
    code = None
    try:
        manifest = verify_frozen_files()
        profile = load_profile()
        speed = profile.load_helper()
        identity = profile.verify_diagnostic_record(BINARY_RECORD)
        storage = speed.verify_storage(BASE)
        # The controller binds at CASE/model/normal-pool/pool.sock. Also keep the
        # timing helper's longer observed/ path check as a conservative bound.
        actual_socket = BASE / CASE / "model/normal-pool/pool.sock"
        actual_socket_bytes = socket_bytes(actual_socket)
        conservative_sockets = speed.socket_preflight(CASE, work_base=BASE)
        if actual_socket_bytes >= SUN_PATH_BYTES:
            raise RuntimeError("actual controller socket path exceeds macOS SUN_LEN")
        if conservative_sockets["normal_pool_socket"]["utf8_bytes_plus_nul"] >= SUN_PATH_BYTES:
            raise RuntimeError("conservative socket path exceeds macOS SUN_LEN")
        activity = speed.active_activity()
        if activity["compilers"] or activity["heavy_mycity"]:
            raise RuntimeError("active compiler/heavy MyCity process blocks readiness: "
                               + json.dumps(activity, sort_keys=True))
        terminal.update({"frozen_manifest_sha256": sha256_file(MANIFEST),
                         "identity": identity, "storage": storage,
                         "socket_paths": {"actual_controller_socket": str(actual_socket),
                                          "actual_utf8_bytes_plus_nul": actual_socket_bytes,
                                          "conservative_profile_check": conservative_sockets},
                         "pre_readiness_processes": activity,
                         "state": "static_preflight_passed"})
        write_json(BASE / "preflight.json", terminal, exclusive=True)

        readiness = {}
        admission, helper_cleanup = profile.guarded_readiness_gate(
            speed, BASE, readiness, terminal)
        write_json(BASE / "readiness-result.json",
                   {"admission": admission, "readiness": readiness,
                    "owned_helper_cleanup": helper_cleanup}, exclusive=True)
        if admission.get("status") != "passed":
            terminal.update({"state": "blocked_admission", "admission": admission,
                             "readiness": readiness, "model_started": False,
                             "controller_launched": False})
            write_json(BASE / "terminal.json", terminal, exclusive=True)
            return 1

        # Fail closed if source, executable, storage, or external activity changed
        # after the full quiet admission window.
        if profile.verify_diagnostic_record(BINARY_RECORD) != identity:
            raise RuntimeError("fixed diagnostic binary/archive identity changed after admission")
        if speed.verify_storage(BASE)["volume"] != storage["volume"]:
            raise RuntimeError("external storage identity changed after admission")
        post_admission_activity = speed.active_activity()
        if post_admission_activity["compilers"] or post_admission_activity["heavy_mycity"]:
            raise RuntimeError("compiler/heavy MyCity activity appeared after admission")
        if verify_frozen_files() != manifest:
            raise RuntimeError("frozen wrapper/controller/support changed after admission")
        if socket_bytes(BASE / CASE / "model/normal-pool/pool.sock") != actual_socket_bytes:
            raise RuntimeError("resolved socket path changed after admission")

        command = [sys.executable, str(CONTROLLER), MODE,
                   "--binary-record", "binary-v128.json", "--case-label", CASE]
        env = {key: value for key, value in os.environ.items()
               if not key.startswith("MYCITY_") and key != "RUST_MIN_STACK"}
        launch = {"state": "launching", "command": command,
                  "source_sha256": identity["source_sha256"],
                  "binary_sha256": identity["binary_sha256"],
                  "source_zip_sha256": identity["source_zip_sha256"],
                  "admission": admission,
                  "scope": intent["scope"],
                  "controller_deadline_seconds": CONTROLLER_DEADLINE_SECONDS,
                  "controller_pid": os.getpid(),
                  "controller_only_peak": speed.sample_controller_peak(),
                  "started_utc": utc_now()}
        write_json(BASE / "launch.json", launch, exclusive=True)
        with (BASE / "controller.stdout").open("x", encoding="utf-8") as out, \
                (BASE / "controller.stderr").open("x", encoding="utf-8") as err:
            child = subprocess.Popen(command, cwd=REPO, stdout=out, stderr=err,
                                     stdin=subprocess.DEVNULL, env=env, close_fds=True)
            launch.update({"state": "running", "controller_child_pid": child.pid})
            write_json(BASE / "launch.json", launch)
            activity_rows, sampling_errors = [], []
            run_started = time.monotonic()
            next_sample = time.monotonic()
            # The fault controller itself owns the unchanged 120 s case deadline.
            # This wrapper waits on that same child and never retries/restarts it.
            while child.poll() is None:
                now = time.monotonic()
                if now >= next_sample:
                    try:
                        rows = speed.ps_rows()
                        excluded = speed.descendant_pids(rows, child.pid)
                        external = speed.process_activity(rows, excluded | {os.getpid()})
                        if external["compilers"] or external["heavy_mycity"]:
                            activity_rows.append({"elapsed_seconds": now-run_started,
                                                  "activity": external})
                    except Exception as exc:
                        sampling_errors.append({"elapsed_monotonic": now,
                                                "error": str(exc)})
                    next_sample = now + 5
                time.sleep(0.25)
            code = child.returncode

        receipt_path = BASE / CASE / "receipt.json"
        case_receipt = None
        receipt_error = None
        try:
            case_receipt = json.loads(receipt_path.read_text(encoding="utf-8"))
        except Exception as exc:
            receipt_error = str(exc)
        passed = controller_result_ok(code, case_receipt,
                                      identity["source_sha256"], identity["binary_sha256"])
        terminal.update({"state": "diagnostic_control_passed_not_acceptance" if passed
                         else "diagnostic_failed_or_incomplete",
                         "model_started": True, "controller_launched": True,
                         "controller_exit_code": code,
                         "controller_receipt_path": str(receipt_path),
                         "controller_receipt": case_receipt,
                         "controller_receipt_error": receipt_error,
                         "external_activity_observed": activity_rows,
                         "external_activity_sampling_errors": sampling_errors,
                         "controller_only_peak": speed.sample_controller_peak(),
                         "observer_peak_accounting": "fault-cases receipt keeps observer already in raw group; wrapper peak is controller-only",
                         "scope": intent["scope"], "ended_utc": utc_now()})
        write_json(BASE / "terminal.json", terminal, exclusive=True)
        return 0 if passed else 1
    except Exception as exc:
        # Do not lose/restart an already launched helper if receipt processing fails.
        if child is not None and code is None:
            try:
                code = child.wait()
            except Exception as wait_exc:
                terminal["wait_error"] = str(wait_exc)
        terminal.update({"state": "blocked_or_failed", "block_reason": str(exc),
                         "model_started": child is not None,
                         "controller_launched": child is not None,
                         "controller_exit_code": code,
                         "ended_utc": utc_now()})
        if not (BASE / "terminal.json").exists():
            write_json(BASE / "terminal.json", terminal, exclusive=True)
        return 1


if __name__ == "__main__":
    try:
        raise SystemExit(run())
    except Exception as exc:
        print(str(exc), file=sys.stderr)
        raise SystemExit(1)
