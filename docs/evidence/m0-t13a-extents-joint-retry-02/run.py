"""Retry-only external R07 topology diagnostic; never formal performance evidence."""
import concurrent.futures
import hashlib
import importlib.util
import json
import os
import subprocess
import sys
import time
import traceback
from pathlib import Path

D = Path(__file__).resolve().parent
R = D.parents[2]
P = D.parent / "m0-t13a-typed-workspace"
W = R / "work/m0-t13a-extents-joint-l1-retry-02"
root = W / "observed"
layout = root / "model/layout"
ackpath = layout / "r07-state.json"
pause = root / "r07-provider-pause"
os.chdir(R)
sys.path.insert(0, str(D))
from controller import (
    assert_stable_context,
    initialize_workspace,
    bind_pinned_baseline,
    topology_for_ack,
    wait_for_context,
    wait_for_existing_child,
)
from preflight import file_sha256, harness_hashes, verify_retry_directory
from topology import assert_pinned_identity, assert_repaid_identity, r07_extents

sys.path.insert(0, str(P))
import native_metrics as native
spec = importlib.util.spec_from_file_location("vmsdk", P / "observe.py")
sdk = importlib.util.module_from_spec(spec)
spec.loader.exec_module(sdk)

info = json.loads((D / "source.json").read_text())
exe = info["binary"]
assert info["source_sha256"] == "32c6e61b9565545177c330f34455611b329fbe1cb38591ec5f6e7879731c4da2"
assert not W.exists(), f"retry workspace already exists: {W}"
stat = os.statvfs(W.parent)
assert stat.f_bavail * stat.f_frsize > 10 * 1024**3


def save(name, value):
    path = D / name
    tmp = path.with_suffix(path.suffix + ".tmp")
    tmp.write_text(json.dumps(value, indent=2) + "\n")
    tmp.replace(path)


save("storage-environment.json", {
    "work_alias": str(R / "work"),
    "work_actual": str((R / "work").resolve(strict=True)),
    "target_actual": str((R / "target").resolve(strict=True)),
    "storage": "external HP P900 USB/APFS SSD",
    "available_bytes_before": stat.f_bavail * stat.f_frsize,
    "source_volume_not_used_for_work_capacity": True,
    "historical_internal_disk_io_comparison": "not_equivalent",
    "formal_performance": False,
})

frozen_preflight = verify_retry_directory(D, R)
save("retry-preflight.json", {
    **frozen_preflight,
    "harness_sha256": harness_hashes(D),
})


def read(path):
    try:
        return json.loads(path.read_text())
    except FileNotFoundError:
        return None


def events():
    path = layout / "events.jsonl"
    if not path.exists():
        return []
    result = []
    for line in path.read_text().splitlines():
        if line.endswith("}"):
            try:
                result.append(json.loads(line))
            except json.JSONDecodeError:
                continue
    return result


def admitted_event(generation):
    for item in events():
        if item.get("snapshot_admitted", {}).get("generation") == generation:
            return item
    return None


def topology(path, expected_mapped=None):
    assert path.suffix == ".jsonl" and path.stat().st_size <= 2 * 1024**2
    rows = [json.loads(line) for line in path.read_text().splitlines()]
    header, footer, body = rows[0], rows[-1], rows[1:-1]
    assert header["record"] == "begin" and footer["record"] == "end" and footer["complete"]
    assert header["owner"] == footer["owner"]
    assert header["epoch"] == footer["epoch"] and header["list_id"] == footer["list_id"]
    assert header["source_sha256"] == info["source_sha256"]
    assert header["expected_ranges"] == footer["actual_ranges"] == len(body)
    assert all(0 < item["len"] <= item["capacity"] and item["start"] > 0 for item in body)
    r07 = [item for item in body if item.get("kind") == "r07_immutable_map"]
    extents = r07_extents({"ranges": body}, expected_mapped) if r07 else []
    return {
        "path": str(path), "sha256": file_sha256(path), "header": header, "footer": footer,
        "ranges": body, "r07_extents": extents,
        "r07_mapped_bytes": sum(item["logical_len"] for item in extents),
    }


def topology_header(path):
    assert path.suffix == ".jsonl" and path.stat().st_size <= 2 * 1024**2
    with path.open() as stream:
        header = json.loads(next(stream))
    assert header.get("record") == "begin"
    return header


def context():
    records = events()
    scans = [item for item in records if "scan" in item]
    lists = [item["vm_regions"] for item in records if "vm_regions" in item]
    if not scans or not lists:
        return None
    ack = read(ackpath)
    if not ack or not isinstance(ack.get("state"), dict):
        return None
    mapped = ack["state"].get("mapped_bytes")
    if not isinstance(mapped, int):
        return None
    list_path = Path(lists[-1]["file"])
    listing = topology_for_ack(ack, list_path, topology_header, topology)
    if listing is None:
        return None
    admitted = [item["snapshot_admitted"]["generation"] for item in records if "snapshot_admitted" in item]
    durable = [item["snapshot_durable"]["generation"] for item in records if "snapshot_durable" in item]
    return {
        "scan": scans[-1],
        "topology": {
            "path": listing["path"], "sha256": listing["sha256"],
            "header": listing["header"], "footer": listing["footer"],
            "ranges": listing["ranges"],
            "r07_extents": listing["r07_extents"],
            "r07_mapped_bytes": listing["r07_mapped_bytes"],
        },
        "ack": ack, "admitted": max(admitted, default=0),
        "durable": max(durable, default=0),
        "phase": "inflight" if max(admitted, default=0) > max(durable, default=0) else "idle",
    }


def waitfor(predicate, child, limit=90):
    end = time.monotonic() + limit
    while time.monotonic() < end:
        value = predicate()
        if value:
            return value
        if child.poll() is not None:
            raise RuntimeError("existing wrapper exited; inspect partial evidence, never restart")
        time.sleep(0.01)
    raise TimeoutError("observation timeout; existing child must be released and awaited")


def identity(pid):
    first = sdk.usage(pid)
    relation = native.relation(pid)
    last = sdk.usage(pid)
    return {
        "first": first, "last": last, "relation": relation,
        "valid": bool(first and last and relation and first["start_abstime"] == last["start_abstime"]),
    }


p = None
controller_error = None
stage = "preflight"
child_exit = None
captures = []
facts = []
pinned_baseline = None
pin_binding = {"claim": "not_made", "reason": "snapshot 1 admission not observed yet"}
snapshot1_admission_event = None
snapshot1_context = None
started = time.monotonic()


def capture(label, role, top, child):
    pid = top["header"]["owner"]["pid"]
    prefix = D / f"{label}-{role}-{pid}"
    before_identity = identity(pid)
    try:
        before_context = wait_for_context(context, label, child)
    except BaseException as exc:
        save(prefix.name + ".json", {
            "label": label, "role": role, "pid": pid,
            "identity_before": before_identity, "context_before": None,
            "context_wait_error": f"{type(exc).__name__}: {exc}",
            "vmmap_started": False, "partial": True,
        })
        raise
    if role == "layout":
        top = before_context["topology"]
        if label == "inflight" and pinned_baseline is not None:
            try:
                assert_pinned_identity(pinned_baseline, top["r07_extents"])
            except AssertionError as exc:
                save(prefix.name + ".json", {
                    "label": label, "role": role, "pid": pid,
                    "context_before": before_context,
                    "pinned_baseline_error": str(exc),
                    "vmmap_started": False, "partial": True,
                })
                raise

    begin = time.monotonic_ns()
    try:
        with prefix.with_suffix(".stdout").open("x") as stdout, prefix.with_suffix(".stderr").open("x") as stderr:
            result = subprocess.run(
                ["/usr/bin/vmmap", "-wide", "-pages", "-interleaved", str(pid)],
                stdout=stdout, stderr=stderr, timeout=20,
            )
            code, error = result.returncode, None
    except subprocess.TimeoutExpired:
        code, error = None, "vmmap_timeout_partial_retained"
    end = time.monotonic_ns()
    after_identity = identity(pid)
    context_error = None
    try:
        # Ack removal during R07 refresh is an expected observation gap. Keep
        # waiting for a valid context, then enforce the original endpoint rule.
        after_context = wait_for_context(context, label, child)
    except BaseException as exc:
        after_context = None
        context_error = f"{type(exc).__name__}: {exc}"

    owner = top["header"]["owner"]
    same_process = bool(
        before_identity["valid"] and after_identity["valid"]
        and before_identity["last"]["start_abstime"]
        == after_identity["last"]["start_abstime"] == owner["start_abstime"]
        and before_identity["relation"] == after_identity["relation"]
    )
    stable = False
    stable_error = None
    pinned_stable = None
    if after_context is not None:
        try:
            assert_stable_context(before_context, after_context, label)
            stable = True
        except AssertionError as exc:
            stable_error = str(exc)
        if label == "inflight" and role == "layout" and pinned_baseline is not None:
            try:
                assert_pinned_identity(pinned_baseline, after_context["topology"]["r07_extents"])
                pinned_stable = True
            except AssertionError as exc:
                pinned_stable = False
                stable_error = f"{stable_error or ''}; pinned identity: {exc}".strip("; ")

    capture_result = {
        "label": label, "role": role, "pid": pid,
        "identity_before": before_identity, "identity_after": after_identity,
        "identity_unchanged": same_process,
        "context_before": before_context, "context_after": after_context,
        "context_wait_error": context_error,
        "topology": top, "tool_begin_monotonic_ns": begin,
        "tool_end_monotonic_ns": end, "exit_code": code, "error": error,
        "stable_phase_epoch_list_endpoints": stable,
        "stable_context_error": stable_error, "pinned_extents_stable": pinned_stable,
        "pin_identity_claim": pin_binding["claim"],
        "stdout": str(prefix.with_suffix(".stdout").relative_to(R)),
        "stderr": str(prefix.with_suffix(".stderr").relative_to(R)),
        "scope": "asynchronous perturbed address attribution only, no physical credit",
    }
    save(prefix.name + ".json", capture_result)
    if context_error:
        raise RuntimeError("could not obtain valid context after vmmap; partial capture saved")
    return capture_result


try:
    stage = "workspace-init"
    root = initialize_workspace(W, (D / "retention-intent.json").read_bytes())
    layout = root / "model/layout"
    ackpath = layout / "r07-state.json"
    pause = root / "r07-provider-pause"
    cmd = [exe, "native-observe", str(root), "--", exe, "r07-layout-driver", str(root / "model"), "--vm-trace"]
    stage = "launch-existing-wrapper"
    with (D / "observer.stdout").open("x") as stdout, (D / "observer.stderr").open("x") as stderr:
        p = subprocess.Popen(cmd, stdout=stdout, stderr=stderr)
    save("running-experiment.json", {"pid": p.pid, "controller_pid": os.getpid(), "command": cmd})

    stage = "initial-layout-observation"
    waitfor(
        lambda: (item if item and item["scan"]["scan"]["iteration"] >= 2
                 and item["ack"]["state"]["epoch"] == item["topology"]["header"]["epoch"]
                 and item["ack"]["state"]["replaced_bytes"] > 0 else None)
        if (item := context()) else None,
        p,
    )
    idle_context = context()
    layout_top = topology(Path(idle_context["topology"]["path"]), idle_context["ack"]["state"]["mapped_bytes"])
    pool_top = topology(root / "model/normal-pool/vm-regions-000000.jsonl")

    stage = "idle-captures"
    with concurrent.futures.ThreadPoolExecutor(max_workers=2) as pool:
        captures.extend(pool.map(lambda pair: capture("idle", *pair, p), [("layout", layout_top), ("pool", pool_top)]))

    stage = "snapshot-1-admission"
    (layout / "vm-start-first").touch(exist_ok=False)
    admission_event = waitfor(lambda: admitted_event(1), p)
    inflight_context = wait_for_context(context, "inflight", p)
    assert inflight_context["admitted"] >= 1
    snapshot1_admission_event = admission_event
    snapshot1_context = inflight_context
    pinned_baseline, pin_binding = bind_pinned_baseline(
        admission_event["snapshot_admitted"], inflight_context
    )
    inflight_layout_top = {
        **inflight_context["topology"],
        "ranges": inflight_context["topology"]["ranges"],
    }
    stage = "snapshot-1-inflight-captures"
    with concurrent.futures.ThreadPoolExecutor(max_workers=2) as pool:
        captures.extend(pool.map(lambda pair: capture("inflight", *pair, p), [("layout", inflight_layout_top), ("pool", pool_top)]))

    stage = "snapshot-2-admission"
    (layout / "vm-start-second").touch(exist_ok=False)
    waitfor(lambda: any(item.get("snapshot_admitted", {}).get("generation") == 2 for item in events()), p)
    bound_context = wait_for_context(context, "inflight", p)
    bound = bound_context["ack"]
    assert bound["state"]["replaced_bytes"] > 0
    bound_topology = bound_context["topology"]
    pause.touch(exist_ok=False)

    stage = "repayment"
    repaid = waitfor(
        lambda: (item if item and item["state"]["replaced_bytes"] == 0 and item["permitted_bytes"] == 0 else None)
        if (item := read(ackpath)) else None,
        p, 10,
    )
    filled_context = wait_for_context(context, "idle", p, timeout=10)
    assert filled_context["topology"]["header"]["epoch"] == repaid["state"]["epoch"]
    filled = topology(Path(filled_context["topology"]["path"]), repaid["state"]["mapped_bytes"])
    assert repaid["state"]["mapped_bytes"] == repaid["state"]["logical_bytes"] - repaid["state"]["arena_tail_bytes"]
    retained = assert_repaid_identity(bound_topology["r07_extents"], filled["r07_extents"], repaid["state"]["mapped_bytes"])
    facts.append({
        "kind": "actual_repaid_extent_topology_validated", "before": bound, "after": repaid,
        "before_extents": bound_topology["r07_extents"], "after_extents": filled["r07_extents"],
        "before_extent_count": len(bound_topology["r07_extents"]), "repaid_extent_count": len(retained),
        "logical_mapped_bytes": filled["r07_mapped_bytes"],
        "topology_path": filled["path"], "topology_sha256": filled["sha256"],
        "preserved_full_extents": "required", "replaced_short_tail_count": "0 or 1 with new generation",
    })
    time.sleep(0.8)
except BaseException as exc:
    controller_error = {"stage": stage, "type": type(exc).__name__, "error": str(exc), "traceback": traceback.format_exc()}
    save("controller-failure.json", {
        **controller_error,
        "source_sha256": info["source_sha256"],
        "child_pid": p.pid if p else None,
        "captures_saved": len(captures),
        "run_restart": False,
    })
finally:
    # Always release both snapshot admission gates and the provider pause before
    # waiting for the one already-launched child; never kill or restart it.
    try:
        if pause.exists():
            pause.unlink()
        if layout.exists():
            (layout / "vm-start-first").touch(exist_ok=True)
            (layout / "vm-start-second").touch(exist_ok=True)
    except BaseException as exc:
        controller_error = controller_error or {
            "stage": "release-markers", "type": type(exc).__name__, "error": str(exc),
            "traceback": traceback.format_exc(),
        }
    if p is not None:
        def wait_receipt(status):
            save("child-wait.json", {
                "child_pid": p.pid, "controller_pid": os.getpid(),
                "child_still_running": status is None, "stage": stage,
                "controller_error": controller_error,
                "note": "markers released; waiting for the existing child; no restart or signal",
            })
        try:
            child_exit = wait_for_existing_child(p, wait_receipt)
        except BaseException as exc:
            controller_error = controller_error or {
                "stage": "wait-existing-child", "type": type(exc).__name__, "error": str(exc),
                "traceback": traceback.format_exc(),
            }
            child_exit = p.poll()
    report = read(root / "observer-report.json") if root.exists() else None
    terminal = {
        "source_sha256": info["source_sha256"], "binary_sha256": info["binary_sha256"],
        "observer_exit": child_exit, "actual_seconds": time.monotonic() - started,
        "captures": captures, "facts": facts, "report": report,
        "controller_error": controller_error, "terminal_stage": stage,
        "snapshot1_admission_event": snapshot1_admission_event,
        "snapshot1_first_valid_inflight_context": snapshot1_context,
        "snapshot1_pin_identity_claim": pin_binding,
        "formal_performance": "not_accepted", "physical_credit_from_VM": 0,
        "real16gb": "not_run", "eight_hour": "not_run",
        "child_was_restarted": False, "child_terminal_wait_completed": child_exit is not None,
    }
    save("terminal.json", terminal)

if controller_error:
    raise SystemExit(1)
assert child_exit == 0 and report and report["status"] == "passed_measurement_wrapper"
assert all(
    item["exit_code"] == 0 and item["identity_unchanged"]
    and item["stable_phase_epoch_list_endpoints"]
    and (item["pinned_extents_stable"] is not False)
    for item in captures
)

# Independent cold restore is a separate evidence phase; retain both snapshots.
env = {key: value for key, value in os.environ.items() if not key.startswith("MYCITY_")}
cold_results = []
for generation in (1, 2):
    snapshot = layout / f"snapshots/snapshot-{generation:06d}"
    cold = W / f"independent-cold-gen{generation}"
    begin = time.monotonic()
    assert snapshot.is_dir()
    with (D / f"independent-cold-gen{generation}.stdout").open("x") as stdout, (D / f"independent-cold-gen{generation}.stderr").open("x") as stderr:
        result = subprocess.run([exe, "layout-load-only", str(snapshot), str(cold)], env=env,
                                stdout=stdout, stderr=stderr, timeout=120)
    receipt = read(cold / "report.json")
    assert result.returncode == 0 and receipt["cold_reopen_and_scan"] == "passed"
    assert receipt["manifest"]["r07"]["replaced_bytes"] == 0
    cold_results.append({
        "generation": generation, "exit": result.returncode,
        "seconds": time.monotonic() - begin, "snapshot": str(snapshot),
        "report": str(cold / "report.json"), "source_sha256": info["source_sha256"],
        "whole_historical_credit_restored": False,
    })
save("cold-result.json", {
    "generations": cold_results, "all_generations_cold_restored": len(cold_results) == 2,
    "source_sha256": info["source_sha256"],
})
print(json.dumps({"observer_exit": child_exit, "captures": len(captures),
                  "stable": True, "cold_generations": len(cold_results),
                  "physical_credit_from_VM": 0}), flush=True)
