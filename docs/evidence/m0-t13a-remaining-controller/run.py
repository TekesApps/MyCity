#!/usr/bin/env python3
"""Execute exactly one fixed-source steady-run through the published observer."""
import hashlib
import json
import os
import re
import subprocess
import sys
import time
from pathlib import Path

sys.dont_write_bytecode = True
HERE = Path(__file__).resolve().parent
REPO = HERE.parents[2]
EVIDENCE = REPO / "docs/evidence"
CONTROL = EVIDENCE / "m0-t13a-joint-smoke-control-03"
SUPPORT_MANIFEST = json.loads((CONTROL / "support-manifest.json").read_text())
SUPPORT_MANIFEST_SHA256 = "aded3e653c0f0da488ffda0a5110441d8dfc1a97987f1d9fdf89642e5adbebe7"
ARCHIVE = EVIDENCE / "m0-t13a-extents-joint-retry-02/frozen-source.zip"
WORK_BASE = REPO / "work"
SOURCE = json.loads((CONTROL / "source.json").read_text())
FREEZE = json.loads((CONTROL / "freeze.json").read_text())
SUPPORT = tuple(SUPPORT_MANIFEST["files"])
PHASES = {"smoke", "formal"}
SCENARIOS = {"l0", "l1", "l2"}


def sha(path):
    h = hashlib.sha256()
    with Path(path).open("rb") as stream:
        for block in iter(lambda: stream.read(1024 * 1024), b""):
            h.update(block)
    return h.hexdigest()


def run_name(scenario, phase, run_id):
    if scenario not in SCENARIOS or phase not in PHASES:
        raise ValueError("scenario must be l0|l1|l2 and phase smoke|formal")
    if phase == "formal":
        if run_id not in {"formal01", "formal02", "formal03"}:
            raise ValueError("formal run-id must be formal01|formal02|formal03")
        suffix = "f" + run_id[-2:]
    else:
        if not re.fullmatch(r"smoke(?:0[1-9]|[1-9][0-9])", run_id):
            raise ValueError("smoke run-id must be smoke01..smoke99")
        suffix = "s" + run_id[5:]
    return f"st-{scenario}-{suffix}"


def formal_plan():
    return [
        {"scenario": scenario, "phase": "formal", "run_id": f"formal{n:02d}",
         "work_path": f"work/{run_name(scenario, 'formal', f'formal{n:02d}')}",
         "state": "planned_not_run"}
        for scenario in ("l0", "l1", "l2") for n in (1, 2, 3)
    ]


def static_contract(archive_path):
    import zipfile
    with zipfile.ZipFile(archive_path) as zf:
        main = zf.read("crates/city-headless/src/main.rs").decode()
        supervisor = zf.read("crates/city-headless/src/steady/supervisor.rs").decode()
        driver = zf.read("crates/city-headless/src/steady/driver.rs").decode()
        client = zf.read("crates/city-headless/src/sim_client.rs").decode()
        steady = zf.read("crates/city-headless/src/steady/mod.rs").decode()
    assert 'usage: steady-run|steady-driver NEW_DIR l0|l1|l2 formal|smoke' in main
    for token in ("warmup: Duration::from_secs(300)", "measure: Duration::from_secs(1800)",
                  "quiesce: Duration::from_secs(10)", "warmup: Duration::from_secs(3)",
                  "measure: Duration::from_secs(12)", "quiesce: Duration::from_secs(1)"):
        assert token in steady, token
    assert "Some(normal_pool::Process::spawn(" in supervisor
    assert '"external_owned_charge_bytes":' in supervisor and "normal_pool::BYTES" in supervisor
    assert "events.wait(plan.total())" in driver
    assert "scale_speed::steady_driver(&measurement, anchor, plan.measure)?" in driver
    assert "normal_pool::broker::Supervisor::register(root)?" in client
    assert "Tier::Full, Mode::Reference" in driver
    return {
        "entrypoint": "steady-run NEW_MODEL_DIR l0|l1|l2 smoke|formal",
        "supervisor_owns_normal_pool": True,
        "l1_is_external_charge_only": True,
        "l2_uses_internal_broker_link": True,
        "duration_contract_seconds": {"formal_warmup": 300, "formal_measured": 1800,
                                       "formal_quiesce": 10},
        "duration_contract_assertion": "from assigned frozen-source contract; runtime not started",
    }


def readiness_gate(exit_code, elapsed_seconds, compiler_seen_elapsed, required_seconds=120):
    """Evaluate only compiler observations in the successful final normal window."""
    window_start = elapsed_seconds - required_seconds
    recent = sorted(t for t in compiler_seen_elapsed if t >= window_start)
    if exit_code != 0:
        status, reason = "failed", "readiness_exit_nonzero"
    elif elapsed_seconds < required_seconds:
        status, reason = "failed", "readiness_window_too_short"
    elif recent:
        status, reason = "failed", "compiler_observed_in_final_120_seconds"
    else:
        status, reason = "passed", None
    return {"status": status, "reason": reason, "required_clean_seconds": required_seconds,
            "window_start_elapsed_seconds": window_start,
            "window_end_elapsed_seconds": elapsed_seconds,
            "recent_compiler_observations_seconds": recent}


def monitored_readiness(command, work):
    """Run the frozen 120-second pressure gate once and retain compiler observations."""
    from process_activity import active_compilers
    records = []
    polls = 0
    began = time.monotonic()
    with (work / "readiness.stdout").open("x") as out, (work / "readiness.stderr").open("x") as err:
        child = subprocess.Popen(command, cwd=REPO, stdout=out, stderr=err)
        while child.poll() is None:
            compilers = active_compilers()
            polls += 1
            if compilers:
                records.append({"elapsed_seconds": time.monotonic() - began,
                                "processes": compilers})
            time.sleep(0.25)
        code = child.returncode
    elapsed = time.monotonic() - began
    gate = readiness_gate(code, elapsed, [r["elapsed_seconds"] for r in records])
    (work / "readiness-compiler-observations.json").write_text(json.dumps({
        "sampling_interval_seconds": 0.25, "process_listing_polls": polls,
        "compiler_observation_count": len(records), "observed_compilers": records,
        "readiness_exit_code": code, "elapsed_seconds": elapsed, "admission": gate,
    }, indent=2) + "\n")
    return code if gate["status"] == "passed" else (code or 1)


def unique_support_hashes():
    if sha(CONTROL / "support-manifest.json") != SUPPORT_MANIFEST_SHA256:
        raise RuntimeError("published support-manifest bytes changed")
    expected = SUPPORT_MANIFEST["files"]
    actual = {name: sha(CONTROL / name) for name in SUPPORT}
    return validate_support_hashes(actual, expected)


def validate_support_hashes(actual, expected):
    if actual != expected:
        mismatches = {name: {"expected": expected.get(name), "actual": actual.get(name)}
                      for name in sorted(set(actual) | set(expected))
                      if actual.get(name) != expected.get(name)}
        raise RuntimeError(f"published 03 support bytes changed: {json.dumps(mismatches, sort_keys=True)}")
    return actual


def clean_mycity_environment(env):
    names = sorted(name for name in env if name.startswith("MYCITY_"))
    for name in names:
        env.pop(name, None)
    return names


def main(argv=None):
    argv = sys.argv[1:] if argv is None else argv
    if len(argv) != 3:
        raise SystemExit("usage: run.py SCENARIO(l0|l1|l2) PHASE(smoke|formal) RUN_ID")
    # Accept the natural CLI order requested by the control spec: scenario phase run-id.
    scenario, phase, run_id = argv
    work_name = run_name(scenario, phase, run_id)
    work = WORK_BASE / work_name
    if work.exists():
        raise SystemExit(f"refusing non-unique run directory: {work}")

    sys.path.insert(0, str(CONTROL))
    from preflight import (verify_freeze, verify_socket_source_inventory, verify_storage)
    from process_activity import active_compilers

    frozen = verify_freeze(SOURCE, FREEZE, ARCHIVE, SOURCE["fixed_binary"], REPO)
    storage = verify_storage(WORK_BASE)
    sockets = verify_socket_source_inventory(ARCHIVE, WORK_BASE, (work_name,))
    contract = static_contract(ARCHIVE)
    compilers = active_compilers()
    if compilers:
        raise SystemExit(json.dumps({"status": "blocked_external_compiler", "processes": compilers}, indent=2))

    support = unique_support_hashes()
    runner_hash = sha(HERE / "run.py")
    plan_path = HERE / "plan.json"
    plan = {
        "schema": "mycity-m0-t13a-remaining-controller-plan-v1",
        "formal_schedule": formal_plan(),
        "single_invocation": {"scenario": scenario, "phase": phase, "run_id": run_id,
                              "work_path": str(work), "state": "preflight_passed_not_run"},
        "source_sha256": SOURCE["fixed_source_sha256"],
        "binary_sha256": SOURCE["fixed_binary_sha256"],
        "support_sha256": support,
        "runner_sha256": runner_hash,
        "limits": {"sampling_hz": 4, "world_bytes": 20 * 1024**3,
                    "combined_log_bytes": 256 * 1024**2, "free_floor_bytes": 2 * 1024**3,
                    "max_non_normal_pressure_seconds": 60,
                    "max_swap_growth_bytes": 256 * 1024**2},
        "retention": {"keep_snapshots_until_cold_restore_and_dependencies_released": True,
                      "no_cleanup_while_run_active": True},
        "formal_duration_seconds": contract["duration_contract_seconds"],
        "status": "preflight_passed_not_run",
    }
    plan_path.write_text(json.dumps(plan, indent=2) + "\n")

    # Recheck compiler absence immediately before the inherited 120-second readiness gate.
    compilers = active_compilers()
    if compilers:
        raise SystemExit(json.dumps({"status": "blocked_external_compiler", "stage": "before readiness",
                                     "processes": compilers}, indent=2))
    work.mkdir()
    observed = work / "observed"
    retention = {
        "phase": phase, "scenario": scenario, "run_id": run_id, "work_name": work_name,
        "source_sha256": SOURCE["fixed_source_sha256"],
        "binary_sha256": SOURCE["fixed_binary_sha256"],
        "retention_policy": plan["retention"],
        "formal_duration_seconds": contract["duration_contract_seconds"],
        "status": "admitted_not_run",
    }
    (work / "preflight.json").write_text(json.dumps({
        "frozen_inputs": frozen, "storage": storage, "socket_paths": sockets,
        "steady_contract": contract, "source_sha256": SOURCE["fixed_source_sha256"],
        "binary_sha256": SOURCE["fixed_binary_sha256"],
        "support_files_sha256": support, "runner_sha256": runner_hash,
        "external_compilers_before_readiness": compilers,
        "status": "passed_static_preflight_only_not_run",
    }, indent=2) + "\n")
    (work / "retention-intent.json").write_text(json.dumps(retention, indent=2) + "\n")
    (work / "measurement-requested").touch(exist_ok=False)
    command = [sys.executable, str(CONTROL / "readiness.py"), str(work)]
    readiness_code = monitored_readiness(command, work)
    (work / "admission.json").write_text(json.dumps({"exit_code": readiness_code}, indent=2) + "\n")
    if readiness_code:
        raise SystemExit(readiness_code)

    # No parked parent: after 120 seconds, revalidate and exec the exact published observer.
    compilers = active_compilers()
    if compilers:
        raise SystemExit(json.dumps({"status": "blocked_external_compiler", "stage": "after readiness",
                                     "processes": compilers}, indent=2))
    assert sha(ARCHIVE) == FREEZE["source_zip_sha256"]
    assert sha(SOURCE["fixed_binary"]) == SOURCE["fixed_binary_sha256"]
    assert support == unique_support_hashes(), "published observer support changed during readiness"
    clean_names = clean_mycity_environment(os.environ)
    model_dir = observed / "model"
    actual_command = [SOURCE["fixed_binary"], "steady-run", str(model_dir), scenario, phase]
    live = {
        "pid": os.getpid(), "scenario": scenario, "phase": phase, "run_id": run_id,
        "command": actual_command, "observer_command": [sys.executable, str(CONTROL / "observe.py"),
                                                         str(observed), "--", *actual_command],
        "source_sha256": SOURCE["fixed_source_sha256"],
        "binary_sha256": SOURCE["fixed_binary_sha256"], "support_files_sha256": support,
        "runner_sha256": runner_hash, "removed_contaminated_mycity_environment_names": clean_names,
        "observer_injected_environment": ["MYCITY_HEAP_LOG", "MYCITY_OBSERVER_CLOCK"],
        "pid_role": "exec observer; no parked orchestration parent",
        "timeout_policy": "wait on this same PID; never automatically restart",
        "external_compiler_policy": "preflight checks; observer retains frame-level compiler evidence",
        "frozen_static": frozen, "storage": storage, "socket_paths": sockets,
        "steady_contract": contract,
    }
    (work / "live.json").write_text(json.dumps(live, indent=2) + "\n")
    observer = [sys.executable, str(CONTROL / "observe.py"), str(observed), "--", *actual_command]
    os.execv(sys.executable, observer)


if __name__ == "__main__":
    main()
