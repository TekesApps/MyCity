#!/usr/bin/env python3
"""Bounded, identity-checked fault cases for the fixed R07 combined-pool binary.

Usage: python3 fault-cases.py {kill-layout|kill-root|half-wire}
Runs once per mode. Case roots are never reused or cleaned. This is a fault
exercise, not a performance or formal acceptance run.
"""
from __future__ import annotations

import hashlib
import importlib.util
import json
import os
import argparse
from pathlib import Path
import selectors
import signal
import socket
import subprocess
import sys
import time
import traceback
from typing import Any

SCRIPT = Path(__file__).resolve()
BASE = SCRIPT.parent
REPO = Path('/Users/frankqin/dev/MyCity')
SUPPORT = REPO / 'docs/evidence/m0-t13a-typed-workspace'
MODES = {'kill-layout', 'kill-root', 'half-wire', 'combined-control', 'separate-control', 'cold-error'}
DEADLINE_SECONDS = 120.0
SAMPLE_PERIOD_SECONDS = 0.25
SOCKET_PATH_LIMIT = 103
MAX_EVENT_TAIL = 1024 * 1024
HALF_WIRE = b'{"source":"pool-control-half-wire"'


def sha256(path: Path) -> str:
    h = hashlib.sha256()
    with path.open('rb') as f:
        for chunk in iter(lambda: f.read(1024 * 1024), b''):
            h.update(chunk)
    return h.hexdigest()


def load_module(name: str, path: Path) -> Any:
    spec = importlib.util.spec_from_file_location(name, path)
    if spec is None or spec.loader is None:
        raise RuntimeError('cannot load support module: ' + str(path))
    module = importlib.util.module_from_spec(spec)
    spec.loader.exec_module(module)
    return module


def write_json(path: Path, value: Any) -> None:
    tmp = path.with_name(path.name + '.tmp')
    with tmp.open('x', encoding='utf-8') as f:
        json.dump(value, f, sort_keys=True, indent=2)
        f.write('\n')
        f.flush()
        os.fsync(f.fileno())
    os.replace(tmp, path)


def read_json(path: Path) -> Any:
    raw = path.read_bytes()
    if len(raw) > 4 * 1024 * 1024:
        raise RuntimeError('JSON evidence exceeds bounded read: ' + str(path))
    return json.loads(raw)


def append_jsonl(path: Path, row: Any) -> None:
    with path.open('a', encoding='utf-8') as f:
        f.write(json.dumps(row, sort_keys=True, separators=(',', ':')) + '\n')
        f.flush()
        os.fsync(f.fileno())


def load_support() -> tuple[Any, Any]:
    observer = load_module('mycity_existing_observe', SUPPORT / 'observe.py')
    native = load_module('mycity_existing_native_metrics', SUPPORT / 'native_metrics.py')
    observer.native = native
    return observer, native


def process_row(native: Any, pid: int) -> tuple[int, Any] | None:
    rows = native.process_table()
    row = rows.get(pid)
    return (pid, row) if row is not None else None


def identity_check(observer: Any, native: Any, pid: int, birth: int,
                   expected_parent: int | None = None) -> dict[str, Any]:
    """Use the frozen usage/relation bracket; never trust a bare PID."""
    found = process_row(native, pid)
    if found is None:
        return {'ok': False, 'reason': 'pid_missing_from_native_process_table',
                'pid': pid, 'expected_birth': birth}
    _, row = found
    usage, issue, proof = observer.bound_usage(pid, row, expected_birth=birth)
    relation = native.relation(pid)
    parent_ok = relation is not None and (expected_parent is None or relation[0] == expected_parent)
    return {'ok': usage is not None and issue is None and parent_ok,
            'pid': pid, 'expected_birth': birth, 'usage': usage, 'issue': issue,
            'relation': relation, 'expected_parent': expected_parent,
            'parent_ok': parent_ok, 'proof': proof}


def current_birth(observer: Any, pid: int) -> int | None:
    sample = observer.usage(pid)
    return None if sample is None else int(sample['start_abstime'])


def read_host_frames(path: Path, offset: int, pending: bytes) -> tuple[list[dict[str, Any]], int, bytes]:
    if not path.exists():
        return [], offset, pending
    with path.open('rb') as f:
        f.seek(offset)
        data = f.read(4 * 1024 * 1024)
        new_offset = f.tell()
    combined = pending + data
    parts = combined.split(b'\n')
    pending = parts.pop()
    frames = []
    for raw in parts:
        if not raw or len(raw) > 1024 * 1024:
            continue
        try:
            frame = json.loads(raw)
        except (UnicodeDecodeError, json.JSONDecodeError):
            continue
        if isinstance(frame, dict):
            frames.append(frame)
    if len(pending) > 1024 * 1024:
        raise RuntimeError('host sample partial-frame bound exceeded')
    return frames, new_offset, pending


def tail_events(path: Path) -> list[dict[str, Any]]:
    if not path.exists():
        return []
    size = path.stat().st_size
    with path.open('rb') as f:
        f.seek(max(0, size - MAX_EVENT_TAIL))
        raw = f.read(MAX_EVENT_TAIL)
    rows = []
    for line in raw.splitlines():
        if len(line) > 65536:
            continue
        try:
            item = json.loads(line)
        except (UnicodeDecodeError, json.JSONDecodeError):
            continue
        if isinstance(item, dict):
            rows.append(item)
    return rows


def member_for(frame: dict[str, Any], role: str) -> list[Any] | None:
    matches = [row for row in frame.get('members', [])
               if isinstance(row, list) and len(row) == 5 and row[4] == role]
    return matches[0] if len(matches) == 1 else None


def r07_state(frame: dict[str, Any]) -> tuple[dict[str, Any], dict[str, Any], dict[str, Any]]:
    applied = frame.get('r07_applied') or {}
    return applied, applied.get('state') or {}, applied.get('owner') or {}


def target_frame_valid(frame: dict[str, Any], source: str,
                       registrations: dict[str, dict[str, Any]]) -> tuple[bool, str]:
    if not frame.get('group_capture_complete') or not frame.get('r07_coverage_complete'):
        return False, 'incomplete_or_uncovered_frame'
    if frame.get('r07_provider_paused') is not False:
        return False, 'provider_not_running'
    applied, state, owner = r07_state(frame)
    layout = registrations.get('layout-r07-session')
    root = registrations.get('r07-layout-driver')
    layout_member = member_for(frame, 'layout-r07-session')
    root_member = member_for(frame, 'r07-layout-driver')
    if not layout or not root or layout_member is None or root_member is None:
        return False, 'registered_root_or_layout_absent'
    if applied.get('source') != source:
        return False, 'r07_source_mismatch'
    if owner.get('pid') != layout['pid'] or owner.get('start_abstime') != layout['birth']:
        return False, 'actual_r07_owner_identity_mismatch'
    if layout_member[0] != layout['pid'] or layout_member[2] != layout['birth']:
        return False, 'layout_member_identity_mismatch'
    if root_member[0] != root['pid'] or root_member[2] != root['birth']:
        return False, 'root_member_identity_mismatch'
    if state.get('mapped_bytes', 0) <= 0 or state.get('replaced_bytes', 0) <= 0:
        return False, 'layout_not_actively_replaced'
    return True, 'active_registered_layout_owner'


def marker_protocol(root: Path, frame: dict[str, Any],
                    protocol: list[dict[str, Any]], source: str,
                    registrations: dict[str, dict[str, Any]],
                    restored_window: list[dict[str, Any]]) -> tuple[list[dict[str, Any]], list[dict[str, Any]]]:
    layout = root / 'model' / 'layout'
    # Re-read the bounded recent tail on every host frame. Predicates are
    # cumulative, so a durable/admitted line cannot be consumed before the
    # corresponding R07 state becomes visible in a later frame.
    rows = tail_events(layout / 'events.jsonl')
    _, state, _ = r07_state(frame)

    def mark(name: str, path: Path, ready: bool) -> None:
        if ready and not any(x.get('step') == name for x in protocol):
            path.touch(exist_ok=False)
            protocol.append({'step': name, 'controller_monotonic_ns': time.monotonic_ns(),
                             'observer_t': frame.get('t'),
                             'r07_epoch': state.get('epoch'),
                             'source_sha256': source})
            write_json(root / 'marker-protocol.json', protocol)

    mark('vm-start-first', layout / 'vm-start-first',
         frame.get('group_capture_complete') is True
         and state.get('replaced_bytes', 0) > 0
         and any((item.get('scan') or {}).get('iteration', 0) >= 2 for item in rows))
    mark('vm-start-second', layout / 'vm-start-second',
         state.get('replaced_bytes', 0) > 0
         and any((item.get('snapshot_durable') or {}).get('generation') == 1 for item in rows))
    pause_marker = root / 'r07-provider-pause'
    mark('r07-provider-pause', pause_marker,
         state.get('replaced_bytes', 0) > 0
         and any((item.get('snapshot_admitted') or {}).get('generation') == 2 for item in rows))
    if pause_marker.exists() and not any(x.get('step') == 'pause-released' for x in protocol):
        applied, state, owner = r07_state(frame)
        root_role = registrations.get('r07-layout-driver')
        layout_role = registrations.get('layout-r07-session')
        root_member = member_for(frame, 'r07-layout-driver')
        layout_member = member_for(frame, 'layout-r07-session')
        footprints = frame.get('member_footprints_pid_birth_bytes') or []
        footprint_sum = sum(row[2] for row in footprints
                            if isinstance(row, list) and len(row) == 3 and isinstance(row[2], int))
        footprint_complete = (len(footprints) == len(frame.get('members', []))
                              and all(isinstance(row, list) and len(row) == 3
                                      and isinstance(row[2], int) for row in footprints))
        restored = bool(
            frame.get('group_capture_complete') is True
            and frame.get('r07_coverage_complete') is True
            and frame.get('r07_provider_paused') is True
            and applied.get('source') == source
            and state.get('mapped_bytes') == 277610496
            and state.get('replaced_bytes') == 0
            and applied.get('permitted_bytes') == 0
            and root_role and layout_role and root_member and layout_member
            and root_member[0] == root_role['pid'] and root_member[2] == root_role['birth']
            and layout_member[0] == layout_role['pid'] and layout_member[2] == layout_role['birth']
            and owner.get('pid') == layout_role['pid']
            and owner.get('start_abstime') == layout_role['birth']
            and footprint_complete
            and footprint_sum == frame.get('phys_footprint_bytes')
        )
        if restored:
            t = frame.get('t')
            if restored_window and (not isinstance(t, (int, float))
                                    or not isinstance(restored_window[-1].get('t'), (int, float))
                                    or not 0 < t - restored_window[-1]['t'] <= 1.0):
                restored_window = []
            restored_window.append(frame)
        else:
            restored_window = []
        if len(restored_window) >= 3:
            selected = restored_window[-3:]
            write_json(root / 'full-restored-window.json', {
                'schema': 'mycity-r07-full-restored-window-v1',
                'source_sha256': source,
                'required_consecutive_frames': 3,
                'predicate': 'group+coverage complete; provider paused; mapped=277610496; replaced=0; permitted=0; exact registered root/layout PID+birth and owner; member footprint sum equals group footprint',
                'frames': selected,
            })
            protocol.append({'step': 'pause-observed', 'controller_monotonic_ns': time.monotonic_ns(),
                             'observer_t': selected[-1].get('t'), 'source_sha256': source,
                             'consecutive_frames': len(selected),
                             'full_restored_window_sha256': sha256(root / 'full-restored-window.json')})
            pause_marker.unlink()
            protocol.append({'step': 'pause-released', 'controller_monotonic_ns': time.monotonic_ns(),
                             'observer_t': selected[-1].get('t'), 'source_sha256': source})
            write_json(root / 'marker-protocol.json', protocol)
    return protocol, restored_window


def registration_state(path: Path, source_expected: str | None = None) -> tuple[dict[str, dict[str, Any]], list[dict[str, Any]], str | None]:
    by_role: dict[str, dict[str, Any]] = {}
    records: list[dict[str, Any]] = []
    nonce: str | None = None
    if not path.exists():
        return by_role, records, nonce
    size = path.stat().st_size
    if size > 16 * 1024 * 1024:
        raise RuntimeError('registration log exceeds observer hard bound')
    for line in path.read_bytes().splitlines():
        if len(line) > 1024:
            raise RuntimeError('registration row exceeds observer hard bound')
        row = json.loads(line)
        if row.get('schema') != 'mycity-observed-process-v1':
            raise RuntimeError('unexpected process registration schema')
        if not isinstance(row.get('nonce'), str) or len(row['nonce']) != 32:
            raise RuntimeError('invalid registration nonce')
        if nonce is not None and row['nonce'] != nonce:
            raise RuntimeError('registration nonce changed')
        nonce = row['nonce']
        if source_expected and row.get('source') != source_expected:
            raise RuntimeError('registration source changed')
        sample = row.get('sample') or {}
        identity = sample.get('identity') or {}
        pid, birth, parent = identity.get('pid'), identity.get('start_abstime'), identity.get('parent_pid')
        if not all(isinstance(x, int) and x > 0 for x in (pid, birth, parent)):
            raise RuntimeError('registration missing exact pid/birth/parent')
        if row.get('kind') == 'started':
            prior = by_role.get(row.get('role'))
            current = {'pid': pid, 'birth': birth, 'parent_pid': parent,
                       'record': row}
            if prior is None or prior['pid'] != pid or prior['birth'] != birth:
                if prior is not None:
                    raise RuntimeError('role was replaced by another process instance: ' + str(row.get('role')))
                by_role[row['role']] = current
        records.append(row)
    return by_role, records, nonce


def all_started(records: list[dict[str, Any]]) -> dict[tuple[int, int], dict[str, Any]]:
    out: dict[tuple[int, int], dict[str, Any]] = {}
    for row in records:
        if row.get('kind') != 'started':
            continue
        ident = row['sample']['identity']
        key = (int(ident['pid']), int(ident['start_abstime']))
        out[key] = {'pid': key[0], 'birth': key[1], 'parent_pid': int(ident['parent_pid']),
                    'role': row.get('role')}
    return out


def terminal_identity_audit(observer: Any, native: Any, identities: dict[tuple[int, int], dict[str, Any]]) -> dict[str, Any]:
    checks = []
    complete = True
    for (pid, birth), info in sorted(identities.items()):
        current = observer.usage(pid)
        if current is None:
            absent = bool(native.confirmed_absent(pid))
            status = 'confirmed_absent' if absent else 'unknown'
            ok = absent
        elif current.get('start_abstime') != birth:
            status = 'pid_reused_new_birth'
            ok = True
        else:
            status = 'same_birth_still_alive'
            ok = False
        checks.append({**info, 'status': status, 'terminal_usage': current,
                       'confirmed_absent': bool(native.confirmed_absent(pid)) if current is None else None})
        complete = complete and ok
    return {'complete': complete, 'identity_count': len(checks), 'identities': checks}


def verify_half_wire(root: Path, source: str) -> dict[str, Any]:
    report_path = root / 'observer-report.json'
    pool_events = root / 'model' / 'normal-pool' / 'events.jsonl'
    layout_report_path = root / 'model' / 'layout' / 'report.json'
    cold_report_path = root / 'model' / 'cold-with-provider' / 'report.json'
    exercise_path = root / 'model' / 'exercise-report.json'
    result: dict[str, Any] = {'checks': {}, 'passed': False}
    if not all(p.exists() for p in (report_path, pool_events, layout_report_path, cold_report_path, exercise_path)):
        result['missing_evidence'] = [str(p) for p in (report_path, pool_events, layout_report_path, cold_report_path, exercise_path) if not p.exists()]
        return result
    observer_report = read_json(report_path)
    exercise = read_json(exercise_path)
    layout_report = read_json(layout_report_path)
    cold_report = read_json(cold_report_path)
    rejects = [x for x in tail_events(pool_events)
               if x.get('kind') == 'rejected'
               and x.get('rejection_reason') == 'normal_pool_control_timeout'
               and (x.get('summary') or {}).get('rejected_requests', 0) >= 1]
    generations = [x.get('generation') for x in layout_report.get('snapshots', [])]
    checks = {
        'observer_passed_wrapper': observer_report.get('status') == 'passed_measurement_wrapper',
        'model_exit_zero': observer_report.get('model_exit_code') == 0,
        'exercise_success_and_source': exercise.get('layout_and_live_provider_cold_exit_zero') is True and exercise.get('source_sha256') == source,
        'pool_timeout_rejected': bool(rejects),
        'layout_report_schema': layout_report.get('schema') == 'mycity-r07-full-session-exercise-v1',
        'two_generations': generations == [1, 2],
        'cold_report_schema': cold_report.get('schema') == 'mycity-layout-cold-reopen-v2',
        'cold_passed': cold_report.get('cold_reopen_and_scan') == 'passed',
        'all_payload_files_hashed_and_loaded': (cold_report.get('restore') or {}).get('all_payload_files_hashed_and_loaded') is True,
        'restored_generation_two': (cold_report.get('restore') or {}).get('restored_generation') == 2,
    }
    result.update(checks=checks, rejection_evidence=rejects[-4:], layout_generations=generations,
                  observer_status=observer_report.get('status'), observer_exit=observer_report.get('model_exit_code'))
    result['passed'] = all(checks.values())
    return result


def main() -> int:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('mode', choices=sorted(MODES))
    parser.add_argument('--binary-record', default='binary-final.json',
                        help='binary identity record relative to this directory (default: binary-final.json)')
    parser.add_argument('--case-label', help='new single directory name for an explicitly reviewed retry')
    args = parser.parse_args()
    mode = args.mode
    label = args.case_label or mode
    if not label or len(label)>24 or any(not (c.isascii() and (c.isalnum() or c=='-')) for c in label):
        raise RuntimeError('case label must be a short ASCII alphanumeric/hyphen directory name')
    if sys.platform != 'darwin':
        raise RuntimeError('this identity-checked runner requires macOS public proc metrics')
    binary_info_path = (BASE / args.binary_record).resolve(strict=True)
    if not binary_info_path.is_relative_to(BASE.resolve(strict=True)):
        raise RuntimeError('binary record must remain inside the attempt directory')
    binary_info = read_json(binary_info_path)
    binary = Path(binary_info['binary'])
    expected_binary_hash = binary_info['binary_sha256']
    expected_source_hash = binary_info.get('source_sha256')
    if (not isinstance(expected_source_hash, str) or len(expected_source_hash) != 64
            or any(ch not in '0123456789abcdef' for ch in expected_source_hash)):
        raise RuntimeError('binary record must pin the exact lowercase source_sha256')
    if not binary.is_absolute() or binary.is_symlink() or not binary.is_file() or not os.access(binary, os.X_OK):
        raise RuntimeError('fixed binary must be absolute, regular, executable, and not a symlink')
    actual_binary_hash = sha256(binary)
    if actual_binary_hash != expected_binary_hash:
        raise RuntimeError('fixed binary hash mismatch')
    canonical_base = BASE.resolve(strict=True)
    case_root = canonical_base / label
    if case_root.exists() or case_root.is_symlink():
        raise RuntimeError('case path already exists; never reuse or overwrite: ' + str(case_root))
    socket_path = (case_root / 'model' / 'normal-pool' / 'pool.sock').resolve(strict=False)
    socket_bytes = os.fsencode(str(socket_path))
    if len(socket_bytes) > SOCKET_PATH_LIMIT:
        raise RuntimeError(f'macOS socket path too long: {len(socket_bytes)} > {SOCKET_PATH_LIMIT}: {socket_path}')

    observer, native = load_support()
    native_source_hash = sha256(SUPPORT / 'native_metrics.py')
    observer_source_hash = sha256(SUPPORT / 'observe.py')
    if sha256(binary) != expected_binary_hash:
        raise RuntimeError('fixed binary changed after preflight')

    argv = [str(binary), 'native-observe', str(case_root), '--', str(binary),
            'r07-layout-driver', str(case_root / 'model')]
    if mode != 'separate-control':
        argv.append('--combined-pool')
    argv.append('--vm-trace')
    env = os.environ.copy()
    for key in list(env):
        if key.startswith('MYCITY_') or key == 'RUST_MIN_STACK':
            env.pop(key, None)
    intent = {
        'schema': 'mycity-controlled-fault-case-intent-v1', 'mode': mode,
        'binary': str(binary), 'binary_sha256': expected_binary_hash,
        'binary_record': str(binary_info_path),
        'binary_record_sha256': sha256(binary_info_path),
        'source_zip_sha256': binary_info.get('source_zip_sha256'),
        'expected_source_sha256': expected_source_hash,
        'argv': argv, 'case_root': str(case_root), 'socket_path': str(socket_path),
        'socket_path_utf8_bytes': len(socket_bytes), 'socket_limit_bytes': SOCKET_PATH_LIMIT,
        'observer_support_sha256': observer_source_hash,
        'native_metrics_support_sha256': native_source_hash,
        'fixed_observer': str(SUPPORT / 'observe.py'), 'fixed_native_metrics': str(SUPPORT / 'native_metrics.py'),
        'workload_or_threshold_changed': False, 'restarts': 0,
        'controller_resource_accounting': 'controller and observer sampled by public proc_pid_rusage; missing remains unknown, never zero',
        'retention': 'retain all snapshots, logs, failed evidence, and receipts; no cleanup',
        'deadline_seconds': DEADLINE_SECONDS,
        'fault': {'kill-layout': 'SIGTERM exact registered layout-r07-session PID/birth only after valid active owner frame',
                  'kill-root': 'SIGKILL exact observer-child root PID/birth only after valid active owner frame',
                  'half-wire': 'send incomplete JSON and close after >2s; require timeout rejection and complete two-snapshot cold report'}[mode],
    }

    # native-observe owns creation of case_root/model; do not pre-create either.
    proc = subprocess.Popen(argv, stdin=subprocess.DEVNULL, stdout=subprocess.PIPE,
                            stderr=subprocess.PIPE, env=env, bufsize=0, close_fds=True)
    observer_pid = proc.pid
    launched_ns = time.monotonic_ns()
    controller_birth = current_birth(observer, os.getpid())
    observer_birth = current_birth(observer, observer_pid)
    if controller_birth is None or observer_birth is None:
        raise RuntimeError('controller or observer exact PID/birth unavailable immediately after launch')
    observer_relation = native.relation(observer_pid)
    if observer_relation is None or observer_relation[0] != os.getpid():
        raise RuntimeError('observer PID/birth parent relation is not this controller')

    selector = selectors.DefaultSelector()
    assert proc.stdout is not None and proc.stderr is not None
    os.set_blocking(proc.stdout.fileno(), False)
    os.set_blocking(proc.stderr.fileno(), False)
    selector.register(proc.stdout, selectors.EVENT_READ, 'stdout')
    selector.register(proc.stderr, selectors.EVENT_READ, 'stderr')
    buffers = {'stdout': bytearray(), 'stderr': bytearray()}
    root_ready = False
    root_created_after_failure = False
    streams: dict[str, Any] = {}
    telemetry = None
    frame_offset = 0
    frame_pending = b''
    protocol: list[dict[str, Any]] = []
    restored_window: list[dict[str, Any]] = []
    seen_events: list[dict[str, Any]] = []
    last_frame: dict[str, Any] | None = None
    valid_owner_proof: dict[str, Any] | None = None
    target_signal: dict[str, Any] | None = None
    wire_attempt: dict[str, Any] | None = None
    wire_socket: socket.socket | None = None
    wire_close_deadline_ns: int | None = None
    fault_injected = False
    timed_out = False
    controller_samples = 0
    controller_peak = 0
    controller_sample_missing = 0
    observer_peak = 0
    observer_sample_missing = 0
    all_process_members: dict[tuple[int, int], dict[str, Any]] = {}
    reason: str | None = None
    source: str | None = None

    def init_root() -> None:
        nonlocal root_ready, telemetry
        if root_ready or not case_root.exists():
            return
        root_ready = True
        write_json(case_root / 'intent.json', intent)
        for name in ('stdout', 'stderr'):
            streams[name] = (case_root / ('native-observe.' + name)).open('xb')
        telemetry = (case_root / 'controller-resource-samples.jsonl').open('x', encoding='utf-8')
        write_json(case_root / 'live.json', {
            'controller_pid': os.getpid(), 'controller_birth': controller_birth,
            'observer_pid': observer_pid, 'observer_birth': observer_birth,
            'observer_parent_relation': observer_relation,
            'launch_monotonic_ns': launched_ns, 'mode': mode,
        })

    try:
        while proc.poll() is None:
            now_ns = time.monotonic_ns()
            init_root()
            if not root_ready and now_ns - launched_ns > int(5e9):
                raise RuntimeError('observer did not create its root within 5s')

            for key, _ in selector.select(timeout=0):
                try:
                    chunk = os.read(key.fileobj.fileno(), 65536)
                except BlockingIOError:
                    continue
                if not chunk:
                    selector.unregister(key.fileobj)
                    continue
                name = key.data
                if not root_ready:
                    if len(buffers[name]) + len(chunk) > 8 * 1024 * 1024:
                        raise RuntimeError('observer output before root exceeds 8MiB')
                    buffers[name].extend(chunk)
                else:
                    streams[name].write(chunk)
                    streams[name].flush()

            if root_ready and telemetry is not None:
                controller = observer.usage(os.getpid())
                observed = observer.usage(observer_pid)
                controller_exact = bool(controller and controller.get('start_abstime') == controller_birth)
                observer_exact = bool(observed and observed.get('start_abstime') == observer_birth)
                controller_bytes = int(controller['phys_footprint_bytes']) if controller_exact else None
                observer_bytes = int(observed['phys_footprint_bytes']) if observer_exact else None
                if not controller_exact:
                    controller_sample_missing += 1
                else:
                    controller_peak = max(controller_peak, controller_bytes)
                if not observer_exact:
                    observer_sample_missing += 1
                else:
                    observer_peak = max(observer_peak, observer_bytes)
                controller_samples += 1
                append_jsonl(case_root / 'controller-resource-samples.jsonl', {
                    'controller_monotonic_ns': now_ns, 'controller': controller,
                    'observer': observed,
                    'controller_identity_exact': controller_exact,
                    'controller_phys_footprint_bytes': controller_bytes,
                    'observer_identity_exact': observer_exact,
                    'observer_phys_footprint_bytes': observer_bytes,
                    'observer_already_in_raw_group': True,
                    'missing_is_not_zero': not controller_exact or not observer_exact,
                })

                frames, frame_offset, frame_pending = read_host_frames(case_root / 'host-samples.jsonl', frame_offset, frame_pending)
                manifest_path = case_root / 'model' / 'manifest.json'
                manifest = read_json(manifest_path) if manifest_path.exists() else {}
                source = manifest.get('source_sha256')
                if source and source != expected_source_hash:
                    raise RuntimeError('binary record and model manifest source hashes differ')
                registrations, reg_rows, nonce = registration_state(case_root / 'registrations.jsonl', source)
                for item in all_started(reg_rows).values():
                    all_process_members[(item['pid'], item['birth'])] = item
                root_sample = read_json(case_root / 'observer-child.json') if (case_root / 'observer-child.json').exists() else None
                if root_sample:
                    root_identity = root_sample.get('identity') or {}
                    root_pid = root_identity.get('pid')
                    root_birth = root_identity.get('start_abstime')
                    registered_root = registrations.get('r07-layout-driver')
                    if registered_root and (registered_root['pid'], registered_root['birth']) != (root_pid, root_birth):
                        raise RuntimeError('observer-child and registration root identity disagree')

                for frame in frames:
                    last_frame = frame
                    for row in frame.get('members', []):
                        if isinstance(row, list) and len(row) == 5 and isinstance(row[2], int) and row[2] > 0:
                            all_process_members[(row[0], row[2])] = {
                                'pid': row[0], 'birth': row[2], 'parent_pid': row[1], 'role': row[4],
                                'first_seen_frame_t': frame.get('t'),
                            }
                    if source:
                        if manifest.get('schema') != ('mycity-r07-full-layout-exercise-v1' if mode=='separate-control' else 'mycity-r07-combined-pool-exercise-v1'):
                            raise RuntimeError('unexpected combined-pool manifest schema')
                        clock_map_path = case_root / 'observer-clock-map.json'
                        if clock_map_path.exists():
                            clock_map = read_json(clock_map_path)
                            if (clock_map.get('supervisor_pid') != root_pid
                                    or clock_map.get('source_sha256') != source):
                                raise RuntimeError('nonce-bound observer clock map source/PID mismatch')
                            request = read_json(case_root/'model/observer-clock-request.json')
                            reply = read_json(case_root/'model/observer-clock-reply.json')
                            if (request.get('pid') != root_pid or reply.get('pid') != root_pid
                                    or request.get('source_sha256') != source or reply.get('source_sha256') != source
                                    or request.get('nonce') != clock_map.get('nonce')
                                    or reply.get('nonce') != clock_map.get('nonce')):
                                raise RuntimeError('clock handshake identities or its own nonce disagree')
                        if frame.get('r07_applied') and (frame['r07_applied'].get('source') != source):
                            raise RuntimeError('frame source differs from model manifest')
                        protocol, restored_window = marker_protocol(
                            case_root, frame, protocol, source, registrations, restored_window)
                    if mode in ('kill-layout', 'kill-root') and not fault_injected and source:
                        complete_protocol = all(any(x.get('step') == step for x in protocol)
                                                for step in ('vm-start-first', 'vm-start-second',
                                                             'r07-provider-pause', 'pause-observed', 'pause-released'))
                        valid, why = target_frame_valid(frame, source, registrations)
                        if valid:
                            root_role = registrations.get('r07-layout-driver')
                            layout_role = registrations.get('layout-r07-session')
                            if not root_role or not layout_role or not root_sample:
                                continue
                            target = layout_role if mode == 'kill-layout' else root_role
                            expected_parent = root_role['pid'] if mode == 'kill-layout' else observer_pid
                            if mode == 'kill-root' and (root_pid, root_birth) != (target['pid'], target['birth']):
                                continue
                            proof = identity_check(observer, native, target['pid'], target['birth'], expected_parent)
                            if not proof['ok']:
                                continue
                            valid_owner_proof = {'frame': frame, 'frame_reason': why,
                                                 'target_identity_check': proof,
                                                 'registrations': {k: v['record'] for k, v in registrations.items()},
                                                 'observer_child': root_sample,
                                                 'protocol': protocol}
                            write_json(case_root / 'fault-target-proof.json', valid_owner_proof)
                            # Recheck exact PID/birth/parent immediately before the one permitted signal.
                            final = identity_check(observer, native, target['pid'], target['birth'], expected_parent)
                            if not final['ok']:
                                raise RuntimeError('target identity changed before signal')
                            sig = signal.SIGTERM if mode == 'kill-layout' else signal.SIGKILL
                            os.kill(target['pid'], sig)
                            target_signal = {'pid': target['pid'], 'birth': target['birth'],
                                             'signal': sig.name, 'controller_monotonic_ns': time.monotonic_ns(),
                                             'pre_signal_identity_check': final,
                                             'target_role': target.get('record', {}).get('role')}
                            write_json(case_root / 'signal-intent.json', target_signal)
                            fault_injected = True
                    if mode == 'cold-error' and not fault_injected and source:
                        valid, why = target_frame_valid(frame, source, registrations)
                        if valid:
                            destination = case_root/'model/cold-with-provider'
                            destination.mkdir(exist_ok=False)
                            write_json(case_root/'cold-error-intent.json', {
                                'path':str(destination),'fault':'preexisting empty output directory makes actual cold child fail',
                                'source_sha256':source,'validated_live_owner_frame':frame,
                                'controller_monotonic_ns':time.monotonic_ns()})
                            fault_injected = True
                    if mode == 'half-wire' and not fault_injected and wire_socket is None and wire_attempt is None:
                        ready_path = case_root / 'model' / 'normal-pool' / 'ready.json'
                        sock_path = case_root / 'model' / 'normal-pool' / 'pool.sock'
                        active_owner, _ = target_frame_valid(frame, source or '', registrations)
                        if active_owner and ready_path.exists() and sock_path.exists():
                            ready = read_json(ready_path)
                            registered = registrations.get('r07-layout-driver')
                            if (ready.get('schema') == 'mycity-normal-pool-v1'
                                    and source and ready.get('source') == source and registered
                                    and ready.get('pid') == registered['pid']):
                                root_check = identity_check(observer, native, registered['pid'],
                                                            registered['birth'], observer_pid)
                                if root_check['ok']:
                                    wire_socket = socket.socket(socket.AF_UNIX, socket.SOCK_STREAM)
                                    wire_socket.settimeout(3.0)
                                    wire_socket.connect(str(sock_path))
                                    wire_socket.sendall(HALF_WIRE)
                                    wire_attempt = {'socket': str(sock_path), 'ready': ready,
                                                    'root_identity_check': root_check,
                                                    'bytes_sent_hex': HALF_WIRE.hex(),
                                                    'connected_monotonic_ns': time.monotonic_ns(),
                                                    'close_after_seconds': 2.25}
                                    wire_close_deadline_ns = time.monotonic_ns() + int(2.25e9)
                                    write_json(case_root / 'half-wire-intent.json', wire_attempt)
                if wire_socket is not None and wire_close_deadline_ns is not None and now_ns >= wire_close_deadline_ns:
                    wire_socket.close()
                    wire_socket = None
                    wire_attempt['closed_monotonic_ns'] = time.monotonic_ns()
                    write_json(case_root / 'half-wire-intent.json', wire_attempt)
                    fault_injected = True
                if now_ns - launched_ns >= int(DEADLINE_SECONDS * 1e9):
                    timed_out = True
                    reason = '120-second safety deadline; observer SIGTERM requested; timeout is never pass'
                    if wire_socket is not None:
                        wire_socket.close()
                        wire_socket = None
                    current = current_birth(observer, observer_pid)
                    relation = native.relation(observer_pid)
                    if current == observer_birth and relation and relation[0] == os.getpid():
                        os.kill(observer_pid, signal.SIGTERM)
                    else:
                        reason += '; observer PID/birth/parent no longer matched, no signal sent'
                    break
                time.sleep(max(0.0, SAMPLE_PERIOD_SECONDS - ((time.monotonic_ns() - now_ns) / 1e9)))

        init_root()
        if not root_ready:
            case_root.mkdir(exist_ok=False)
            root_created_after_failure = True
            root_ready = True
            write_json(case_root / 'intent.json', intent)
            for name in ('stdout', 'stderr'):
                streams[name] = (case_root / ('native-observe.' + name)).open('xb')
            telemetry = (case_root / 'controller-resource-samples.jsonl').open('x', encoding='utf-8')
        for key, _ in selector.select(timeout=0):
            while True:
                try:
                    chunk = os.read(key.fileobj.fileno(), 65536)
                except BlockingIOError:
                    break
                if not chunk:
                    selector.unregister(key.fileobj)
                    break
                streams[key.data].write(chunk)
        for name, stream in streams.items():
            if buffers[name]:
                stream.write(buffers[name])
            stream.flush()
            os.fsync(stream.fileno())
            stream.close()
        exit_code = proc.wait()
    except BaseException as exc:
        reason = reason or f'{type(exc).__name__}: {exc}'
        if root_ready:
            try:
                write_json(case_root / 'controller-error.json', {'error': reason, 'traceback': traceback.format_exc()})
            except Exception:
                pass
        # A controller error must not abandon the live observer/root. If root is
        # alive, verified observer termination closes its private owner pipe and
        # the existing root guard kills its owned group. If root was already
        # killed by the requested fault, let the native observer clean descendants.
        cleanup = {'requested': False, 'observer_pid': observer_pid, 'reason': 'controller_error'}
        if proc.poll() is None:
            exact = identity_check(observer, native, observer_pid, observer_birth, os.getpid())
            cleanup['observer_identity'] = exact
            root_live = True
            if 'root_pid' in locals() and root_pid is not None:
                root_usage = observer.usage(root_pid)
                root_live = bool(root_usage and root_usage.get('start_abstime') == root_birth)
            cleanup['root_still_live'] = root_live
            if exact['ok'] and root_live:
                os.kill(observer_pid, signal.SIGTERM)
                cleanup['requested'] = True
                cleanup['signal'] = 'SIGTERM'
        try:
            proc.wait(timeout=10)
        except subprocess.TimeoutExpired:
            cleanup['observer_still_live_after_wait'] = True
        cleanup['observer_exit_code'] = proc.poll()
        if root_ready:
            write_json(case_root/'controller-error-cleanup.json', cleanup)
        exit_code = proc.poll()
    finally:
        try:
            selector.close()
        except Exception:
            pass
        for stream in streams.values():
            try:
                if not stream.closed:
                    stream.close()
            except Exception:
                pass
        if telemetry is not None:
            telemetry.flush()
            telemetry.close()

    if not root_ready:
        raise RuntimeError('observer root unavailable; no evidence directory was created')
    manifest_path = case_root / 'model' / 'manifest.json'
    if manifest_path.exists():
        source = read_json(manifest_path).get('source_sha256')
    source_matches_fixed_binary = source == expected_source_hash
    registrations, reg_rows, nonce = registration_state(case_root / 'registrations.jsonl', source)
    identity_roster = all_started(reg_rows)
    for key, value in all_process_members.items():
        identity_roster.setdefault(key, value)
    lifecycle = terminal_identity_audit(observer, native, identity_roster)
    observer_report = read_json(case_root / 'observer-report.json') if (case_root / 'observer-report.json').exists() else None
    exercise_report = read_json(case_root / 'model' / 'exercise-report.json') if (case_root / 'model' / 'exercise-report.json').exists() else None
    if mode in ('combined-control','separate-control'):
        outcome = verify_half_wire(case_root, source or '')
        outcome.get('checks',{}).pop('pool_timeout_rejected',None)
        pool_path = case_root/'model/normal-pool/report.json'
        pool_report = read_json(pool_path) if pool_path.exists() else {}
        outcome['checks']['original_pool_balanced'] = (pool_report.get('balanced_after_owned_wait') is True
            and pool_report.get('summary',{}).get('available_touched_bytes') == 67108864
            and pool_report.get('summary',{}).get('granted_capacity_bytes') == 0)
        passed = bool(not timed_out and outcome.get('checks') and all(outcome['checks'].values())
            and lifecycle.get('complete') and source_matches_fixed_binary)
    elif mode == 'cold-error':
        cold_started = [row for row in reg_rows if row.get('kind')=='started' and row.get('role')=='layout-load-only']
        cold_stderr = case_root/'model/cold.stderr'
        outcome = {'cold_child_actually_registered':bool(cold_started),
            'cold_failure_output':cold_stderr.read_text() if cold_stderr.exists() else None,
            'no_successful_exercise':exercise_report is None,
            'observer_model_exit_code':observer_report.get('model_exit_code') if observer_report else None}
        passed = bool(fault_injected and not timed_out and cold_started and observer_report
            and observer_report.get('status')=='failed' and observer_report.get('model_exit_code')==2
            and exercise_report is None and lifecycle.get('complete') and source_matches_fixed_binary)
    elif mode == 'half-wire':
        outcome = verify_half_wire(case_root, source or '')
        passed = bool(fault_injected and not timed_out and outcome.get('passed')
                      and lifecycle.get('complete') and source_matches_fixed_binary)
    else:
        successful_exercise = bool(exercise_report and exercise_report.get('layout_and_live_provider_cold_exit_zero') is True)
        passed = bool(fault_injected and target_signal and not timed_out and observer_report
                      and observer_report.get('status') != 'passed_measurement_wrapper'
                      and observer_report.get('model_exit_code') != 0
                      and not successful_exercise and lifecycle.get('complete')
                      and source_matches_fixed_binary)
        outcome = {'observer_status': None if observer_report is None else observer_report.get('status'),
                   'observer_model_exit_code': None if observer_report is None else observer_report.get('model_exit_code'),
                   'successful_exercise_report': successful_exercise,
                   'signal_issued': target_signal is not None,
                   'all_registered_and_observed_identities_terminal': lifecycle.get('complete')}
    receipt = {
        'schema': 'mycity-controlled-fault-case-receipt-v1', 'mode': mode,
        'passed_controlled_exercise':passed, 'passed_fault_exercise': passed if mode not in ('combined-control','separate-control') else False, 'performance_acceptance': 'not_claimed',
        'fault_injected': fault_injected, 'timed_out': timed_out,
        'reason': reason, 'observer_exit_code': exit_code,
        'controller_pid': os.getpid(), 'controller_birth': controller_birth,
        'observer_pid': observer_pid, 'observer_birth': observer_birth,
        'source_sha256': source, 'binary_sha256': expected_binary_hash,
        'source_sha256_matches_fixed_binary': source_matches_fixed_binary,
        'registration_nonce': nonce, 'registration_record_count': len(reg_rows),
        'target_signal': target_signal, 'half_wire': wire_attempt,
        'fault_target_proof_present': valid_owner_proof is not None,
        'marker_protocol': protocol, 'protocol_complete': all(any(x.get('step') == step for x in protocol)
            for step in ('vm-start-first', 'vm-start-second', 'r07-provider-pause', 'pause-observed', 'pause-released')),
        'identity_audit': lifecycle, 'outcome_checks': outcome,
        'observer_report': observer_report, 'exercise_report': exercise_report,
        'controller_auxiliary': {
            'sample_count': controller_samples, 'missing_identity_samples': controller_sample_missing,
            'controller_only_peak_phys_footprint_bytes': controller_peak if controller_sample_missing == 0 else None,
            'unavailable_controller_samples_not_zero': True,
            'sample_log': str(case_root / 'controller-resource-samples.jsonl'),
        },
        'observer_measurement': {
            'sample_count': controller_samples, 'missing_identity_samples': observer_sample_missing,
            'peak_phys_footprint_bytes': observer_peak if observer_sample_missing == 0 else None,
            'already_in_raw_group': True,
            'must_not_be_added_to_raw_group_again': True,
        },
        'case_root_created_after_observer_failure': root_created_after_failure,
        'retention': 'all files retained; no snapshot cleanup or restart',
        'terminal_monotonic_ns': time.monotonic_ns(),
    }
    write_json(case_root / 'receipt.json', receipt)
    print(json.dumps(receipt, indent=2, sort_keys=True))
    return 0 if passed else 1


if __name__ == '__main__':
    raise SystemExit(main())
