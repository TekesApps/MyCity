#!/usr/bin/env python3
"""One guarded speed-run wrapper; refuses launch without the observer clock contract."""
import hashlib
import importlib.util
import json
import os
import re
import subprocess
import sys
from pathlib import Path

sys.dont_write_bytecode = True
HERE = Path(__file__).resolve().parent
SPEEDS = ("600", "60", "10", "1", "unpaced")
RUN = (HERE / "run.py").resolve()
_spec = importlib.util.spec_from_file_location("remaining_steady_controller", RUN)
steady = importlib.util.module_from_spec(_spec)
_spec.loader.exec_module(steady)


def speed_name(rate, run_id):
    if rate not in SPEEDS:
        raise ValueError("rate must be 600|60|10|1|unpaced")
    if not re.fullmatch(r"speed(?:0[1-9]|[1-9][0-9])", run_id):
        raise ValueError("run-id must be speed01..speed99")
    token = "u" if rate == "unpaced" else rate
    return f"sp-{token}-{run_id[-2:]}"


def speed_plan():
    return [{"rate": rate, "run_id": "speed01", "work_path": f"work/{speed_name(rate, 'speed01')}",
             "state": "planned_not_run"} for rate in SPEEDS]


def validate_source_identity(source, expected_source, expected_binary):
    if (source.get("fixed_source_sha256") != expected_source
            or source.get("fixed_binary_sha256") != expected_binary):
        raise RuntimeError("speed source or binary identity mismatch")
    return {"source_sha256": expected_source, "binary_sha256": expected_binary,
            "matches_fixed_identity": True}


def inspect_speed_contract(archive_path):
    """Check the actual speed-run path and prove whether it joins observer clock exchange."""
    import zipfile
    with zipfile.ZipFile(archive_path) as archive:
        main = archive.read("crates/city-headless/src/main.rs").decode()
        speed = archive.read("crates/city-headless/src/scale_speed.rs").decode()
        rust = {name: archive.read(name).decode("utf-8", "replace")
                for name in archive.namelist() if name.endswith(".rs")}
    observer = (steady.CONTROL / "observe.py").read_text()
    clock_client = (steady.CONTROL / "clock_exchange.py").read_text()
    expected = (
        '"speed-run" | "speed-driver"',
        'mycity_headless::scale_speed::run(std::path::Path::new(&args[2]), rate)?',
        'mycity_headless::scale_speed::driver(std::path::Path::new(&args[2]), rate)?',
    )
    for fragment in expected:
        assert fragment in main, f"frozen speed entrypoint changed: {fragment}"
    for fragment in ('"unpaced" => None', '"600" => Some(600)',
                     '"60" => Some(60)', '"10" => Some(10)', '"1" => Some(1)'):
        assert fragment in speed, f"frozen speed rate missing: {fragment}"
    for fragment in ("game_days\":30", "hours\":720", "targets\":723",
                     "windows != 724", "scale_model_task::run(&dir, Tier::Full, Mode::Reference)?",
                     "scale_model_task::run(&dir, Tier::Full, Mode::BeforeReopen)?",
                     "Mode::CompareSpeed", '"L1":"absent"'):
        assert fragment in speed, f"frozen speed workload contract changed: {fragment}"
    clock_calls = []
    for name, text in rust.items():
        for number, line in enumerate(text.splitlines(), 1):
            if "observer_clock::synchronize" in line:
                clock_calls.append({"source": name, "line": number, "text": line.strip()})
    speed_path_has_clock_sync = "crate::observer_clock::synchronize" in speed
    observer_requires_exchange = ("clock_exchange.exchange(" in observer
                                  and "mycity-observer-clock-request-v1" in clock_client
                                  and "nonce" in clock_client)
    return {
        "entrypoint": "speed-run NEW_MODEL_DIR 600|60|10|1|unpaced",
        "selected_entrypoint": "speed-run",
        "speed_run_dispatch": "scale_speed::run; supervisor creates its own normal_pool and starts speed-driver",
        "workload": {"full_days": 30, "hours": 720, "targets": 723, "worker_windows": 724,
                     "cpu_reference_before_operating": True, "l1": "absent",
                     "common_8mib": "not_accepted", "eight_hour": "not_run",
                     "full_m0": "not_accepted", "completion_is_speed_acceptance": False},
        "observer_clock_handshake": {
            "observer_requires_nonce_bound_startup_exchange": observer_requires_exchange,
            "speed_module_calls_synchronize": speed_path_has_clock_sync,
            "synchronize_call_sites_in_fixed_source": clock_calls,
            "compatible": speed_path_has_clock_sync and observer_requires_exchange,
            "reason": None if speed_path_has_clock_sync else
                "speed-run and speed-driver do not call observer_clock::synchronize; published observer waits for observer-clock-ready.json and fails the nonce-bound exchange",
        },
        "model_launch_permitted": speed_path_has_clock_sync and observer_requires_exchange,
    }


def main(argv=None):
    argv = sys.argv[1:] if argv is None else argv
    if len(argv) != 2:
        raise SystemExit("usage: speed_run.py RATE(600|60|10|1|unpaced) RUN_ID(speed01..speed99)")
    rate, run_id = argv
    work_name = speed_name(rate, run_id)
    work = steady.WORK_BASE / work_name
    if work.exists():
        raise SystemExit(f"refusing non-unique run directory: {work}")
    identity = validate_source_identity(steady.SOURCE, steady.SOURCE["fixed_source_sha256"],
                                        steady.SOURCE["fixed_binary_sha256"])
    sys.path.insert(0, str(steady.CONTROL))
    from preflight import verify_freeze, verify_socket_source_inventory, verify_storage
    from process_activity import active_compilers
    frozen = verify_freeze(steady.SOURCE, steady.FREEZE, steady.ARCHIVE,
                           steady.SOURCE["fixed_binary"], steady.REPO)
    storage = verify_storage(steady.WORK_BASE)
    sockets = verify_socket_source_inventory(steady.ARCHIVE, steady.WORK_BASE, (work_name,))
    support = steady.unique_support_hashes()
    # The guard runs before any model/readiness launch and never weakens clock integrity.
    contract = inspect_speed_contract(steady.ARCHIVE)
    receipt = {"identity": identity, "source_sha256": identity["source_sha256"],
               "binary_sha256": identity["binary_sha256"], "rate": rate,
               "planned_work_path": str(work), "contract": contract,
               "frozen_input_preflight": frozen, "storage_preflight": storage,
               "socket_paths": sockets, "support_files_sha256": support,
               "state": "blocked_prelaunch_not_run" if not contract["model_launch_permitted"]
                       else "preflight_pending_not_run"}
    (HERE / "speed-clock-compatibility.json").write_text(json.dumps(receipt, indent=2) + "\n")
    if not contract["model_launch_permitted"]:
        raise SystemExit("blocked before readiness/model launch: frozen speed entrypoint lacks observer clock handshake")

    if active_compilers():
        raise SystemExit("external compiler active before readiness; retained without stopping")
    work.mkdir()
    observed = work / "observed"
    (work / "preflight.json").write_text(json.dumps({
        "source_preflight": frozen, "storage": storage, "socket_paths": sockets,
        "source_sha256": identity["source_sha256"], "binary_sha256": identity["binary_sha256"],
        "support_files_sha256": support, "speed_contract": contract,
        "state": "passed_static_preflight_not_run",
    }, indent=2) + "\n")
    (work / "retention-intent.json").write_text(json.dumps({
        "rate": rate, "run_id": run_id, "source_sha256": identity["source_sha256"],
        "binary_sha256": identity["binary_sha256"],
        "scope": "fixed full 30-day speed workload, L1 absent; no 8-hour or full-M0 claim",
        "no_cleanup_while_active": True, "state": "admitted_not_run",
    }, indent=2) + "\n")
    (work / "measurement-requested").touch(exist_ok=False)
    readiness = [sys.executable, str(steady.CONTROL / "readiness.py"), str(work)]
    code = steady.monitored_readiness(readiness, work)
    (work / "admission.json").write_text(json.dumps({"exit_code": code}, indent=2) + "\n")
    if code:
        raise SystemExit(code)
    if active_compilers():
        raise SystemExit("external compiler active after readiness; retained without stopping")
    if support != steady.unique_support_hashes():
        raise SystemExit("published support identity changed during readiness")
    actual = [steady.SOURCE["fixed_binary"], "speed-run", str(observed / "model"), rate]
    clean_names = steady.clean_mycity_environment(os.environ)
    (work / "live.json").write_text(json.dumps({
        "pid": os.getpid(), "command": actual, "observer": str(steady.CONTROL / "observe.py"),
        "source_sha256": identity["source_sha256"], "binary_sha256": identity["binary_sha256"],
        "support_files_sha256": support, "removed_contaminated_mycity_environment_names": clean_names,
        "pid_role": "exec observer; no parked parent",
        "restart_policy": "never restart automatically", "frozen_contract": contract,
        "frozen_preflight": frozen, "storage": storage, "socket_paths": sockets,
    }, indent=2) + "\n")
    os.execv(sys.executable, [sys.executable, str(steady.CONTROL / "observe.py"),
                              str(observed), "--", *actual])


if __name__ == "__main__":
    main()
