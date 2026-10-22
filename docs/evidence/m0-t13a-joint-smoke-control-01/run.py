#!/usr/bin/env python3
"""Run one fresh fixed-source joint smoke/formal trial; never restarts a run."""
import hashlib
import json
import os
import re
import subprocess
import sys
import time
from pathlib import Path

from log_budget import analyze as analyze_log_budget
from preflight import verify_freeze, verify_storage
from process_activity import active_compilers

HERE = Path(__file__).resolve().parent
REPO = HERE.parents[2]
EVIDENCE = REPO / "docs/evidence"
SOURCE = json.loads((HERE / "source.json").read_text())
FREEZE = json.loads((HERE / "freeze.json").read_text())
ARCHIVE = EVIDENCE / "m0-t13a-extents-joint-retry-02/frozen-source.zip"
WORK_BASE = REPO / "work"
SUPPORT_FILES = (
    "observe.py", "native_metrics.py", "clock_exchange.py", "readiness.py",
    "process_activity.py", "preflight.py", "log_budget.py", "run.py",
    "source.json", "freeze.json",
)
PHASE_PREFIX = {
    "smoke": "m0-t13a-joint-smoke-control-01-smoke-600",
    "formal": "m0-t13a-joint-smoke-control-01-formal-28800",
}


def sha(path):
    h = hashlib.sha256()
    with Path(path).open("rb") as stream:
        for block in iter(lambda: stream.read(1024 * 1024), b""):
            h.update(block)
    return h.hexdigest()


def fail_if_compilers(stage):
    active = active_compilers()
    if active:
        raise SystemExit(json.dumps({"status": "blocked_external_compiler", "stage": stage, "processes": active}, indent=2))
    return active


def support_hashes():
    return {name: sha(HERE / name) for name in SUPPORT_FILES}


def check_successful_smoke(smoke_id):
    smoke_root = WORK_BASE / f"{PHASE_PREFIX['smoke']}-{smoke_id}"
    observed = smoke_root / "observed"
    wrapper = json.loads((observed / "observer-report.json").read_text())
    model = json.loads((observed / "model/report.json").read_text())
    live = json.loads((smoke_root / "live.json").read_text())
    assert wrapper["status"] == "passed_measurement_wrapper", "smoke observer did not complete successfully"
    assert wrapper["model_exit_code"] == 0
    assert model.get("schema") == "mycity-joint-endurance-v1"
    assert model.get("source_sha256") == SOURCE["fixed_source_sha256"]
    assert live.get("phase") == "smoke" and live.get("source_sha256") == SOURCE["fixed_source_sha256"]
    assert live.get("binary_sha256") == SOURCE["fixed_binary_sha256"]
    assert live.get("support_files_sha256") == support_hashes(), "fixed support/source proof changed since smoke"
    assert model.get("cadence") == "smoke"
    assert model.get("business_and_layout_completed") is True
    assert model.get("layout_scans", 0) >= 600
    budget = analyze_log_budget(smoke_root)
    saved_budget_path = smoke_root / "log-budget.json"
    assert saved_budget_path.exists(), "run log_budget.py after smoke and review its persisted receipt before formal"
    saved_budget = json.loads(saved_budget_path.read_text())
    assert saved_budget == budget, "saved log-budget receipt no longer matches the complete smoke frames"
    assert budget["cap_estimate"] == "conservative_projection_under_hard_cap", (
        "conservative 8h log upper projection exceeds the unchanged cap; retain this smoke and do not start formal"
    )
    return {
        "path": str(smoke_root),
        "observer_status": wrapper["status"],
        "model_report": str(observed / "model/report.json"),
        "source_sha256": SOURCE["fixed_source_sha256"],
        "binary_sha256": SOURCE["fixed_binary_sha256"],
        "support_files_sha256": live["support_files_sha256"],
        "log_budget_receipt": str(saved_budget_path),
        "log_budget_upper_estimate_bytes": budget["formal_combined_log_upper_estimate_bytes"],
        "log_budget_cap_estimate": budget["cap_estimate"],
    }


def main():
    if len(sys.argv) not in (3, 4) or sys.argv[1] not in PHASE_PREFIX:
        raise SystemExit("usage: run.py smoke|formal RUN_ID [SUCCESSFUL_SMOKE_RUN_ID]")
    phase, run_id = sys.argv[1:3]
    if not re.fullmatch(r"[a-z0-9][a-z0-9-]{0,31}", run_id):
        raise SystemExit("RUN_ID must be 1-32 lowercase letters, digits, or hyphens")
    if phase == "formal":
        if len(sys.argv) != 4:
            raise SystemExit("formal requires the successful smoke RUN_ID")
        smoke_receipt = check_successful_smoke(sys.argv[3])
    else:
        if len(sys.argv) != 3:
            raise SystemExit("smoke does not accept a prior smoke RUN_ID")
        smoke_receipt = None

    # Verify immutable inputs, volume identity and available capacity before creating a run.
    static = verify_freeze(SOURCE, FREEZE, ARCHIVE, SOURCE["fixed_binary"], REPO)
    storage = verify_storage(WORK_BASE)
    static["storage"] = storage
    static["phase"] = phase
    static["prior_smoke"] = smoke_receipt
    static["preparation_repo_commit"] = SOURCE["prepared_repo_commit"]
    static["current_repo_head"] = subprocess.check_output(["git", "rev-parse", "HEAD"], cwd=REPO, text=True).strip()
    static["external_compilers_at_static_preflight"] = active_compilers()
    (HERE / f"preflight-{phase}-{run_id}.json").write_text(json.dumps(static, indent=2) + "\n")

    fail_if_compilers("before normal-pressure admission")
    work = WORK_BASE / f"{PHASE_PREFIX[phase]}-{run_id}"
    work.mkdir()  # New evidence parent only. Rust/observer own their respective `observed` roots.
    observed = work / "observed"
    if observed.exists():
        raise SystemExit(f"refusing pre-created observer/model root: {observed}")
    (work / "measurement-requested").touch(exist_ok=False)
    retention = {
        "phase": phase,
        "run_id": run_id,
        "source_sha256": SOURCE["fixed_source_sha256"],
        "binary_sha256": SOURCE["fixed_binary_sha256"],
        "new_work_root": str(work),
        "new_observed_root_must_be_created_by_observer": True,
        "retain_both_complete_snapshots_until_actual_cold_restore_and_dependency_release": True,
        "retain_protected_190_originals": str(REPO / SOURCE["retention"]["protected_original_inventory"]),
        "no_cleanup_while_run_active": True,
        "estimated_three_generations_plus_combined_logs_bytes": 3 * 3_378_784_402 + 256 * 1024**2,
        "combined_log_hard_cap_bytes": 256 * 1024**2,
        "world_hard_cap_bytes": 20 * 1024**3,
    }
    (work / "retention-intent.json").write_text(json.dumps(retention, indent=2) + "\n")

    began = time.monotonic()
    readiness_command = [sys.executable, str(HERE / "readiness.py"), str(work)]
    with (work / "readiness.stdout").open("x") as out, (work / "readiness.stderr").open("x") as err:
        code = subprocess.run(readiness_command, cwd=REPO, stdout=out, stderr=err).returncode
    (work / "admission.json").write_text(json.dumps({"exit_code": code, "seconds": time.monotonic() - began}, indent=2) + "\n")
    if code:
        raise SystemExit(code)

    # Recheck after the full 120s window. Never begin the measured workload alongside Cargo.
    fail_if_compilers("after normal-pressure admission and before observer exec")
    clean_names = sorted(name for name in os.environ if name.startswith("MYCITY_"))
    for name in clean_names:
        os.environ.pop(name, None)
    command = [
        sys.executable,
        str(HERE / "observe.py"),
        str(observed),
        "--",
        SOURCE["fixed_binary"],
        "joint-endurance",
        str(observed / "model"),
        phase,
    ]
    live = {
        "pid": os.getpid(),
        "phase": phase,
        "run_id": run_id,
        "command": command,
        "source_sha256": SOURCE["fixed_source_sha256"],
        "binary_sha256": SOURCE["fixed_binary_sha256"],
        "support_files_sha256": support_hashes(),
        "removed_contaminated_mycity_environment_names": clean_names,
        "observer_injected_environment": ["MYCITY_HEAP_LOG", "MYCITY_OBSERVER_CLOCK"],
        "pid_role": "exec observer in place after admission; no parked orchestration parent",
        "timeout_policy": "none; wait on this same PID until terminal; never restart automatically",
        "cargo_policy": "blocked at admission and launch; any compiler observed after launch remains recorded per frame; no process is stopped",
        "prior_smoke": smoke_receipt,
    }
    (work / "live.json").write_text(json.dumps(live, indent=2) + "\n")
    os.execv(sys.executable, command)


if __name__ == "__main__":
    main()
