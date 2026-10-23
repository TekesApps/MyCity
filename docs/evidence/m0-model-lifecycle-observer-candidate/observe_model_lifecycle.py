#!/usr/bin/env python3
"""Read-only, bounded macOS observer for the normal-pool Model lifecycle CLI.

This script never starts/stops a MyCity process. Run it concurrently with the
separately launched, source-pinned `normal-pool-model-diagnostic` command.
"""
from __future__ import annotations

import argparse
import ctypes
import hashlib
import importlib
import json
import os
from pathlib import Path
import resource
import sys
import time
import tracemalloc

SUPPORT_DIR = Path("/Users/frankqin/dev/MyCity/docs/evidence/m0-t13a-joint-smoke-control-03")
SUPPORT_SHA256 = {
    "observe.py": "0d7ddad946762bb3a9b683caa71dcf99cda6de08ff14c45c6877757e0d468d5d",
    "native_metrics.py": "4a672f7c711f30250ec493f63dad6ba95895e97ec5d10edcc99d034eb989051e",
    "process_activity.py": "1abbc0771180936fa660b34db4f6428f199fd467e0f866548e8a12bdb4c58aa9",
    "frame_codec.py": "005be0c42f1703e0aae4d6e7597d83e65654ffb805f22456873ae042e4a9b97f",
}
MAX_EVENT_BYTES = 64 * 2048
MAX_EVENT_ROW_BYTES = 2048
MAX_OUTPUT_BYTES = 256 * 1024
MAX_OUTPUT_ROWS = 256
MAX_LINE_BYTES = 8192
MAX_RUNTIME_SECONDS = 620.0
PHASES = ("prepared", "filled", "retiring", "returned")
PHASE_KINDS = {
    "prepared": "prepared_before_helper_spawn",
    "filled": "helper_actual_compare_slots_filled",
    "retiring": "retiring_after_helper_wait_or_cancel",
    "returned": "returned_after_helper_wait_or_cancel",
}


class OutputCapacityReached(RuntimeError):
    pass


def sha256(path: Path) -> str:
    digest = hashlib.sha256()
    with path.open("rb") as stream:
        while chunk := stream.read(65536):
            digest.update(chunk)
    return digest.hexdigest()


def json_file(path: Path, cap: int) -> dict:
    info = path.lstat()
    if not path.is_file() or path.is_symlink() or info.st_size > cap:
        raise RuntimeError(f"bounded regular JSON file required: {path.name}")
    value = json.loads(path.read_bytes())
    if not isinstance(value, dict):
        raise RuntimeError(f"JSON object required: {path.name}")
    return value


class EventTail:
    """Incremental, row- and byte-bounded reader for the append-only event log."""

    def __init__(self, path: Path):
        self.path = path
        self.inode: tuple[int, int] | None = None
        self.offset = 0
        self.pending = bytearray()
        self.total_bytes = 0
        self.rows = 0
        self.events: list[dict] = []

    def refresh(self) -> None:
        try:
            info = self.path.lstat()
        except FileNotFoundError:
            return
        if self.path.is_symlink() or not self.path.is_file():
            raise RuntimeError("event log must remain a regular file")
        inode = (info.st_dev, info.st_ino)
        if self.inode is None:
            self.inode = inode
        elif self.inode != inode or info.st_size < self.offset:
            raise RuntimeError("event log replaced or truncated")
        delta = info.st_size - self.offset
        if delta < 0 or self.total_bytes + delta > MAX_EVENT_BYTES:
            raise RuntimeError("event log byte cap exceeded")
        with self.path.open("rb") as stream:
            stream.seek(self.offset)
            chunk = stream.read(delta)
        if len(chunk) != delta:
            raise RuntimeError("event log changed during bounded read")
        self.offset += len(chunk)
        self.total_bytes += len(chunk)
        self.pending.extend(chunk)
        while True:
            newline = self.pending.find(b"\n")
            if newline < 0:
                if len(self.pending) >= MAX_EVENT_ROW_BYTES:
                    raise RuntimeError("event row cap exceeded")
                break
            row = bytes(self.pending[: newline + 1])
            del self.pending[: newline + 1]
            if len(row) > MAX_EVENT_ROW_BYTES:
                raise RuntimeError("event row cap exceeded")
            self.rows += 1
            if self.rows > 64:
                raise RuntimeError("event row count cap exceeded")
            value = json.loads(row)
            if not isinstance(value, dict):
                raise RuntimeError("event row must be object")
            if value.get("owner_kind") == "model" and value.get("phase") in PHASES:
                self.events.append(value)

    @staticmethod
    def fingerprint(event: dict) -> tuple:
        ident = event.get("identity") or {}
        return (
            event.get("phase"), event.get("generation"), event.get("observed_ns"),
            event.get("pid"), ident.get("process_instance_unix_ns"),
        )

    @property
    def latest(self) -> dict | None:
        return self.events[-1] if self.events else None


class Writer:
    def __init__(self, path: Path):
        if path.exists() or path.is_symlink():
            raise RuntimeError("observer output must be a fresh path")
        path.parent.resolve(strict=True)
        self.stream = path.open("xb")
        self.rows = 0
        self.bytes = 0

    def write(self, record: dict, *, terminal: bool = False) -> None:
        raw = (json.dumps(record, sort_keys=True, separators=(",", ":")) + "\n").encode()
        if len(raw) > MAX_LINE_BYTES:
            raise RuntimeError("observer record line cap exceeded")
        # Reserve one row and one maximum-size line for an explicit terminal
        # record, so reaching either cap is reported without erasing prior rows.
        row_limit = MAX_OUTPUT_ROWS if terminal else MAX_OUTPUT_ROWS - 1
        byte_limit = MAX_OUTPUT_BYTES if terminal else MAX_OUTPUT_BYTES - MAX_LINE_BYTES
        if self.rows >= row_limit or self.bytes + len(raw) > byte_limit:
            if terminal:
                raise RuntimeError("reserved terminal record cannot fit output cap")
            raise OutputCapacityReached("256-row or 256-KiB output budget reached")
        self.stream.write(raw)
        self.stream.flush()
        self.rows += 1
        self.bytes += len(raw)

    def close(self) -> None:
        self.stream.close()


def import_support(support: Path):
    for name, expected in SUPPORT_SHA256.items():
        path = support / name
        if sha256(path) != expected:
            raise RuntimeError(f"pinned observer support hash mismatch: {name}")
    sys.path.insert(0, str(support))
    native = importlib.import_module("native_metrics")
    observe = importlib.import_module("observe")
    activity = importlib.import_module("process_activity")
    observe.native = native
    return native, observe, activity


def task_info(native, pid: int) -> dict | None:
    value = native.Task()
    got = native.lib.proc_pidinfo(pid, native.TASK, 0, ctypes.byref(value), ctypes.sizeof(value))
    if got == 0:
        return None
    if got != ctypes.sizeof(value):
        raise RuntimeError("proc_pidinfo taskinfo size mismatch")
    names = {name for name, _kind in native.Task._fields_}
    required = {"pti_virtual_size", "pti_resident_size", "pti_threadnum"}
    if not required.issubset(names):
        raise RuntimeError("pinned taskinfo ABI lacks required fields")
    return {
        "virtual_bytes": int(value.pti_virtual_size),
        "resident_bytes": int(value.pti_resident_size),
        "thread_count": int(value.pti_threadnum),
        "method": "proc_pidinfo(PROC_PIDTASKINFO)",
    }


def capture_process(native, observe, pid: int, expected_parent: int, expected_exe: Path,
                    known_births: dict[tuple[int, int], int]) -> dict:
    key = (pid, expected_parent)
    begin = time.monotonic_ns()
    path_a = native.process_path(pid)
    rel_a = native.relation(pid)
    usage_a = observe.usage(pid)
    task = task_info(native, pid)
    usage_b = observe.usage(pid)
    rel_b = native.relation(pid)
    path_b = native.process_path(pid)
    end = time.monotonic_ns()
    if not path_a or not path_b or not rel_a or not rel_b or not usage_a or not usage_b or task is None:
        return {"pid": pid, "state": "missing", "reason": "native_identity_or_metrics_unavailable",
                "capture_begin_monotonic_ns": begin, "capture_end_monotonic_ns": end,
                "start_abstime": None, "rss_bytes": None, "phys_footprint_bytes": None,
                "task_vm": None}
    exe_a = Path(path_a).resolve()
    exe_b = Path(path_b).resolve()
    birth_a = usage_a.get("start_abstime")
    birth_b = usage_b.get("start_abstime")
    if (birth_a is None or birth_a != birth_b or rel_a[0] != expected_parent or
            rel_b[0] != expected_parent or rel_a[3] != rel_b[3] or exe_a != exe_b or exe_a != expected_exe):
        return {"pid": pid, "state": "identity_mismatch", "reason": "pid_parent_exe_or_birth_changed",
                "capture_begin_monotonic_ns": begin, "capture_end_monotonic_ns": end,
                "start_abstime": birth_a, "rss_bytes": None, "phys_footprint_bytes": None,
                "task_vm": None}
    prior = known_births.get(key)
    if prior is not None and prior != birth_a:
        return {"pid": pid, "state": "identity_mismatch", "reason": "os_process_start_changed_for_pid",
                "capture_begin_monotonic_ns": begin, "capture_end_monotonic_ns": end,
                "start_abstime": birth_a, "rss_bytes": None, "phys_footprint_bytes": None,
                "task_vm": None}
    known_births[key] = birth_a
    return {
        "pid": pid,
        "state": "captured",
        "parent_pid": expected_parent,
        "process_name": rel_b[3],
        "executable_path": str(exe_b),
        "os_process_start_abstime": birth_a,
        "os_process_start_clock": "Mach absolute time from proc_pid_rusage(ri_proc_start_abstime); not application instance",
        "capture_begin_monotonic_ns": begin,
        "capture_end_monotonic_ns": end,
        "rss_bytes": usage_b.get("rss_bytes"),
        "phys_footprint_bytes": usage_b.get("phys_footprint_bytes"),
        "task_vm": task,
        "stack_or_vm_region_resident_bytes": None,
        "stack_or_vm_region_reason": "not exposed by the reused bounded proc_pid_rusage/proc_pidinfo taskinfo provider",
    }


def expected_event_sequence(tail: EventTail) -> tuple[list[str], dict | None]:
    phases = []
    filled = None
    generation = None
    for event in tail.events:
        phase = event.get("phase")
        if event.get("schema") != "mycity-normal-role-arena-v2":
            raise RuntimeError("unexpected lifecycle event schema")
        if event.get("kind") != PHASE_KINDS[phase]:
            raise RuntimeError("phase/event kind mismatch")
        if not phases:
            if phase != "prepared" or event.get("pid") is not None or event.get("identity") is not None:
                raise RuntimeError("invalid prepared lifecycle identity")
        elif len(phases) >= len(PHASES) or phase != PHASES[len(phases)]:
            raise RuntimeError("unexpected lifecycle phase ordering")
        current_generation = event.get("generation")
        if not isinstance(current_generation, int) or current_generation <= 0:
            raise RuntimeError("invalid lifecycle generation")
        if generation is None:
            generation = current_generation
        elif generation != current_generation:
            raise RuntimeError("lifecycle generation changed")
        if phase == "filled":
            ident = event.get("identity")
            pid = event.get("pid")
            if not isinstance(ident, dict) or not isinstance(pid, int) or ident.get("process_pid") != pid:
                raise RuntimeError("invalid filled process identity")
            app_instance = ident.get("process_instance_unix_ns")
            if not isinstance(app_instance, int) or app_instance <= 0:
                raise RuntimeError("invalid application instance identity")
            filled = {"pid": pid, "application_instance_unix_ns": app_instance,
                      "generation": event.get("generation")}
        elif phase in ("retiring", "returned"):
            ident = event.get("identity")
            if not filled or event.get("pid") != filled["pid"] or not isinstance(ident, dict):
                raise RuntimeError("helper identity changed across lifecycle")
            if ident.get("process_instance_unix_ns") != filled["application_instance_unix_ns"]:
                raise RuntimeError("application instance changed across lifecycle")
        phases.append(phase)
    return phases, filled


def system_context(native, observe, activity, rows: dict) -> dict:
    try:
        compilers = activity.observer_compilers(rows, native.process_path)
        compiler_rows = [{k: item.get(k) for k in ("pid", "ppid", "comm", "executable_path")}
                         for item in compilers[:16]]
        enum_failures = len(native.LAST_ENUMERATION_FAILURES)
        compiler_result = {"count": len(compilers) if enum_failures == 0 else None,
                           "partial_count": len(compilers), "rows": compiler_rows,
                           "rows_truncated": len(compilers) > 16,
                           "complete": enum_failures == 0,
                           "enumeration_failure_count": enum_failures}
    except Exception as exc:
        compiler_result = {"count": None, "rows": None,
                           "missing_reason": type(exc).__name__}
    try:
        pressure = observe.pressure()
    except Exception as exc:
        pressure = {"value": None, "missing_reason": type(exc).__name__}
    try:
        swap = observe.sysctl("vm.swapusage", observe.Swap())
        swap_result = {"used_bytes": int(swap.used), "total_bytes": int(swap.total),
                       "method": "sysctl vm.swapusage"}
    except Exception as exc:
        swap_result = {"used_bytes": None, "total_bytes": None,
                       "missing_reason": type(exc).__name__}
    return {"system_context_only_not_acceptance_gate": {
        "pressure_dispatch_flags": pressure, "swap": swap_result,
        "external_compilers": compiler_result, "process_table_count": len(rows)}}


def event_view(event: dict) -> dict:
    identity = event.get("identity") or {}
    return {
        "phase": event.get("phase"), "kind": event.get("kind"),
        "generation": event.get("generation"), "observed_ns": event.get("observed_ns"),
        "transition_ns": event.get("transition_ns"), "pid": event.get("pid"),
        "application_instance_unix_ns": identity.get("process_instance_unix_ns"),
        "capacity_bytes": event.get("capacity_bytes"),
        "pool_touched_bytes": event.get("pool_touched_bytes"),
        "summary": event.get("summary"),
    }


def captured_phase_names(captured_phases: set[str]) -> list[str]:
    """Only confirmed native captures belong here; attempts are separate."""
    return sorted(captured_phases)


def main() -> int:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--run-root", required=True, type=Path)
    parser.add_argument("--output", required=True, type=Path)
    parser.add_argument("--supervisor-pid", required=True, type=int)
    parser.add_argument("--exe", required=True, type=Path)
    parser.add_argument("--exe-sha256", required=True)
    parser.add_argument("--source-sha256", required=True)
    parser.add_argument("--support-dir", type=Path, default=SUPPORT_DIR)
    parser.add_argument("--poll-ms", type=int, default=50)
    parser.add_argument("--filled-sample-seconds", type=float, default=1.0)
    args = parser.parse_args()
    if args.poll_ms < 25 or args.poll_ms > 250 or not (0.5 <= args.filled_sample_seconds <= 5.0):
        parser.error("poll/sample interval outside fixed observer bounds")
    run_root = args.run_root
    if not run_root.is_absolute() or run_root.parent.resolve(strict=True) / run_root.name != run_root:
        parser.error("run root must be a canonical absolute child of an existing parent")
    if args.output.resolve(strict=False).parent != args.output.parent.resolve(strict=True):
        parser.error("output parent must be canonical")
    if args.output.resolve(strict=False).is_relative_to(run_root):
        parser.error("observer output must be outside the fresh model run tree")
    exe = args.exe.resolve(strict=True)
    actual_exe_sha = sha256(exe)
    if actual_exe_sha != args.exe_sha256:
        parser.error("fresh executable SHA-256 does not match pin")
    if len(args.source_sha256) != 64 or any(c not in "0123456789abcdef" for c in args.source_sha256):
        parser.error("source SHA-256 must be lowercase hex")

    tracemalloc.start(1)
    writer = Writer(args.output)
    start_ns = time.monotonic_ns()
    identity_births: dict[tuple[int, int], int] = {}
    attempted_fingerprints: set[tuple] = set()
    captured_phases: set[str] = set()
    captured_sample_count = 0
    attempted_sample_count = 0
    tail: EventTail | None = None
    terminal = {"status": "running", "reason": None}
    native = observe = activity = None
    ready_seen = False
    pool_application_identity: tuple[int, int, str] | None = None
    report_seen = False
    filled_last = 0.0
    last_phase_count = 0

    def emit(record: dict, *, terminal: bool = False) -> None:
        record.setdefault("observer_elapsed_monotonic_ns", time.monotonic_ns() - start_ns)
        writer.write(record, terminal=terminal)

    try:
        native, observe, activity = import_support(args.support_dir.resolve(strict=True))
        if sha256(args.support_dir / "observe.py") != SUPPORT_SHA256["observe.py"]:
            raise RuntimeError("observer support changed after import")
        emit({"type": "observer_start", "status": "running",
              "observer_sha256": sha256(Path(__file__).resolve()),
              "executable_sha256": actual_exe_sha, "expected_source_sha256": args.source_sha256,
              "support_sha256": SUPPORT_SHA256, "supervisor_pid": args.supervisor_pid,
              "poll_ms": args.poll_ms, "filled_sample_seconds": args.filled_sample_seconds,
              "caps": {"runtime_seconds": MAX_RUNTIME_SECONDS, "event_bytes": MAX_EVENT_BYTES,
                       "event_rows": 64, "output_bytes": MAX_OUTPUT_BYTES,
                       "output_rows": MAX_OUTPUT_ROWS, "line_bytes": MAX_LINE_BYTES}})

        while time.monotonic_ns() - start_ns < int(MAX_RUNTIME_SECONDS * 1e9):
            loop_begin = time.monotonic_ns()
            if not run_root.exists():
                time.sleep(args.poll_ms / 1000)
                continue
            if run_root.is_symlink() or not run_root.is_dir():
                raise RuntimeError("run root is not a fresh regular directory")
            pool_root = run_root / "normal-pool"
            if not pool_root.exists():
                time.sleep(args.poll_ms / 1000)
                continue
            if tail is None:
                tail = EventTail(pool_root / "events.jsonl")
            tail.refresh()
            ready_path = pool_root / "ready.json"
            if ready_path.exists():
                ready = json_file(ready_path, 4096)
                if (not isinstance(ready.get("pid"), int) or
                        not isinstance(ready.get("boot"), int) or ready.get("boot") <= 0 or
                        ready.get("source") != args.source_sha256):
                    # ready.pid identifies the pool child, not the CLI supervisor.
                    raise RuntimeError("ready PID/source does not match pinned supervisor/source")
                if ready.get("capacity_bytes") != 64 * 1024 * 1024 or not ready.get("boot"):
                    raise RuntimeError("ready pool capacity/application instance mismatch")
                ready_identity = (ready["pid"], ready["boot"], ready["source"])
                if pool_application_identity is not None and pool_application_identity != ready_identity:
                    raise RuntimeError("pool PID/application instance/source changed during run")
                pool_application_identity = ready_identity
                ready_seen = True

            phases, filled = expected_event_sequence(tail)
            latest = tail.latest
            now = time.monotonic()
            due = latest is not None and (
                EventTail.fingerprint(latest) not in attempted_fingerprints or
                (latest.get("phase") == "filled" and now - filled_last >= args.filled_sample_seconds)
            )
            if due:
                before = EventTail.fingerprint(latest)
                attempted_fingerprints.add(before)
                attempted_sample_count += 1
                captured_ns = time.monotonic_ns()
                try:
                    rows = native.process_table()
                    phase = latest["phase"]
                    pool_pid = int(ready["pid"]) if ready_seen else None
                    if pool_pid is None:
                        raise RuntimeError("pool ready identity unavailable")
                    supervisor_relation = native.relation(args.supervisor_pid)
                    if supervisor_relation is None:
                        raise RuntimeError("supervisor relation unavailable")
                    supervisor_row = rows.get(args.supervisor_pid)
                    if supervisor_row is None or supervisor_row[0] != supervisor_relation[0]:
                        raise RuntimeError("supervisor missing or changed in native process enumeration")
                    pool_row = rows.get(pool_pid)
                    if pool_row is None or pool_row[0] != args.supervisor_pid:
                        raise RuntimeError("pool missing or changed in native process enumeration")
                    supervisor_sample = capture_process(
                        native, observe, args.supervisor_pid, supervisor_relation[0], exe,
                        identity_births)
                    pool_sample = capture_process(native, observe, pool_pid, args.supervisor_pid,
                                                  exe, identity_births)
                    helper_sample = None
                    expected_helper_state = "not_spawned"
                    if phase == "filled":
                        if filled is None:
                            raise RuntimeError("filled event has no bound application identity")
                        helper_row = rows.get(filled["pid"])
                        if helper_row is None or helper_row[0] != args.supervisor_pid:
                            raise RuntimeError("Model helper missing or changed in native process enumeration")
                        expected_helper_state = "present"
                        helper_sample = capture_process(native, observe, filled["pid"],
                                                        args.supervisor_pid, exe, identity_births)
                    elif phase in ("retiring", "returned"):
                        if filled is None:
                            raise RuntimeError("retirement without filled helper identity")
                        if native.confirmed_absent(filled["pid"]):
                            helper_sample = {"pid": filled["pid"], "state": "confirmed_absent",
                                             "reason": "helper_waited_before_retirement_event",
                                             "os_process_start_abstime": None, "rss_bytes": None,
                                             "phys_footprint_bytes": None}
                            expected_helper_state = "confirmed_absent"
                        else:
                            expected_helper_state = "expected_absent_but_not_confirmed"
                            helper_sample = capture_process(native, observe, filled["pid"],
                                                            args.supervisor_pid, exe, identity_births)
                    else:
                        expected_helper_state = "not_spawned"
                        helper_sample = {"pid": None, "state": "not_started",
                                         "os_process_start_abstime": None, "rss_bytes": None,
                                         "phys_footprint_bytes": None}
                    event_context = system_context(native, observe, activity, rows)
                    self_usage = observe.usage(os.getpid())
                    current_alloc, peak_alloc = tracemalloc.get_traced_memory()
                    self_context = {
                        "observer_self": {
                            "pid": os.getpid(),
                            "ru_maxrss_bytes": int(resource.getrusage(resource.RUSAGE_SELF).ru_maxrss),
                            "tracemalloc_current_bytes": current_alloc,
                            "tracemalloc_peak_bytes": peak_alloc,
                            "native_phys_footprint_bytes": self_usage.get("phys_footprint_bytes") if self_usage else None,
                            "native_rss_bytes": self_usage.get("rss_bytes") if self_usage else None,
                            "native_missing_reason": None if self_usage else "proc_pid_rusage_unavailable",
                        }
                    }
                    tail.refresh()
                    after = EventTail.fingerprint(tail.latest) if tail.latest else None
                    if before != after:
                        emit({"type": "sample_rejected", "status": "missing",
                              "reason": "lifecycle_event_advanced_during_native_capture",
                              "event_before": event_view(latest),
                              "event_after": event_view(tail.latest) if tail.latest else None,
                              "native_capture_discarded": True})
                    else:
                        required_captures = [supervisor_sample, pool_sample]
                        if phase == "filled":
                            required_captures.append(helper_sample)
                        elif phase in ("retiring", "returned"):
                            required_captures.append(helper_sample)
                        native_capture_complete = (
                            all(item is not None and item.get("state") == "captured"
                                for item in required_captures[:2]) and
                            (phase == "prepared" or
                             (phase == "filled" and helper_sample is not None and
                              helper_sample.get("state") == "captured") or
                             (phase in ("retiring", "returned") and helper_sample is not None and
                              helper_sample.get("state") == "confirmed_absent")))
                        emit({"type": "phase_sample",
                              "status": "captured" if native_capture_complete else "identity_incomplete",
                              "native_capture_complete": native_capture_complete,
                              "sampling_semantics": "processes captured sequentially with per-process identity brackets; not a synchronized cross-process peak",
                              "phase": phase, "event": event_view(latest),
                              "event_fingerprint": list(before),
                              "capture_begin_monotonic_ns": captured_ns,
                              "capture_end_monotonic_ns": time.monotonic_ns(),
                              "pool_application_identity": {
                                  "pid": ready["pid"],
                                  "application_instance_unix_ns": ready["boot"],
                                  "source_sha256": ready["source"]},
                              "supervisor": supervisor_sample,
                              "pool": pool_sample,
                              "model_helper_expected_state": expected_helper_state,
                              "model_helper": helper_sample,
                              **event_context, **self_context})
                        if native_capture_complete:
                            captured_phases.add(phase)
                            captured_sample_count += 1
                        if phase == "filled":
                            filled_last = now
                except Exception as exc:
                    emit({"type": "phase_sample", "status": "missing",
                          "reason": type(exc).__name__ + ":" + str(exc)[:256],
                          "event": event_view(latest), "native_metrics": None,
                          "numbers_substituted_with_zero": False})
                    if latest.get("phase") == "filled":
                        filled_last = now

            if tail and len(tail.events) > last_phase_count:
                last_phase_count = len(tail.events)

            report_path = run_root / "model-lifecycle-diagnostic.json"
            if report_path.exists():
                report = json_file(report_path, 16384)
                if (report.get("schema") != "mycity-normal-pool-model-lifecycle-diagnostic-v1" or
                        report.get("source_sha256") != args.source_sha256 or
                        report.get("pool", {}).get("pid") != ready.get("pid") or
                        report.get("pool", {}).get("source_sha256") != args.source_sha256):
                    raise RuntimeError("final diagnostic report source or PID mismatch")
                helper = report.get("model_helper", {})
                if phases != list(PHASES):
                    raise RuntimeError("final report present without all four ordered lifecycle events")
                if filled and (helper.get("pid") != filled["pid"] or
                               helper.get("application_instance_unix_ns") != filled["application_instance_unix_ns"] or
                               helper.get("source_sha256") != args.source_sha256 or
                               helper.get("result_source_validated_by_scale_model_task") is not True):
                    raise RuntimeError("final helper report does not bind to filled event")
                report_seen = True
                pool_absent = native.confirmed_absent(int(report["pool"]["pid"]))
                if pool_absent:
                    samples_complete = set(PHASES).issubset(captured_phases)
                    terminal = {"status": "diagnostic_capture_complete" if samples_complete
                                else "diagnostic_capture_incomplete",
                                "reason": "report_present_and_pool_confirmed_absent",
                                "report_acceptance": report.get("acceptance"),
                                "phases_seen": phases,
                                "lifecycle_four_events": phases == list(PHASES),
                                "native_four_phase_samples": samples_complete,
                                "cli_exit_code": "recorded_separately_by_shell_wait",
                                "captured_sample_count": captured_sample_count,
                                "attempted_sample_count": attempted_sample_count,
                                "native_region_stack": "not captured; see per-process reason"}
                    # The raw lifecycle phase list is authoritative; samples may
                    # be missing if a phase raced past the observer.
                    emit({"type": "observer_terminal", **terminal,
                          "event_phases": phases,
                          "captured_phase_names": captured_phase_names(captured_phases),
                          "diagnostic_is_not_memory_or_performance_acceptance": True}, terminal=True)
                    return 0
            if time.monotonic_ns() - start_ns >= int(MAX_RUNTIME_SECONDS * 1e9):
                break
            spent = (time.monotonic_ns() - loop_begin) / 1e9
            time.sleep(max(0.0, args.poll_ms / 1000 - spent))

        terminal = {"status": "incomplete", "reason": "observer_timeout_or_run_not_finished"}
        emit({"type": "observer_terminal", **terminal,
              "ready_seen": ready_seen, "report_seen": report_seen,
              "event_phases": [e.get("phase") for e in tail.events] if tail else [],
              "captured_phase_names": captured_phase_names(captured_phases),
              "lifecycle_four_events": bool(tail and [e.get("phase") for e in tail.events] == list(PHASES)),
              "native_four_phase_samples": set(PHASES).issubset(captured_phases),
              "captured_sample_count": captured_sample_count,
              "attempted_sample_count": attempted_sample_count,
              "cli_exit_code": "recorded_separately_by_shell_wait",
              "diagnostic_is_not_memory_or_performance_acceptance": True})
        return 2
    except OutputCapacityReached as exc:
        emit({"type": "observer_terminal", "status": "diagnostic_capture_incomplete",
              "reason": str(exc), "ready_seen": ready_seen, "report_seen": report_seen,
              "event_phases": [e.get("phase") for e in tail.events] if tail else [],
              "captured_phase_names": captured_phase_names(captured_phases),
              "lifecycle_four_events": bool(tail and [e.get("phase") for e in tail.events] == list(PHASES)),
              "native_four_phase_samples": set(PHASES).issubset(captured_phases),
              "captured_sample_count": captured_sample_count,
              "attempted_sample_count": attempted_sample_count,
              "cli_exit_code": "recorded_separately_by_shell_wait",
              "diagnostic_is_not_memory_or_performance_acceptance": True}, terminal=True)
        return 2
    except Exception as exc:
        try:
            emit({"type": "observer_terminal", "status": "fail_closed",
                  "reason": type(exc).__name__ + ":" + str(exc)[:512],
                  "captured_phase_names": captured_phase_names(captured_phases),
                  "lifecycle_four_events": bool(tail and [e.get("phase") for e in tail.events] == list(PHASES)),
                  "native_four_phase_samples": set(PHASES).issubset(captured_phases),
                  "captured_sample_count": captured_sample_count,
                  "attempted_sample_count": attempted_sample_count,
                  "cli_exit_code": "recorded_separately_by_shell_wait",
                  "numbers_substituted_with_zero": False,
                  "diagnostic_is_not_memory_or_performance_acceptance": True}, terminal=True)
        except Exception:
            pass
        return 2
    finally:
        writer.close()


if __name__ == "__main__":
    raise SystemExit(main())
