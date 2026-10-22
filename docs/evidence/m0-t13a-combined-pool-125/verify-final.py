#!/usr/bin/env python3
"""Read-only, report-level verification for the finished pool-control evidence.

Does not open/hash snapshot payload files, change evidence, run workloads, or
interpret incomplete/full-run directories as completed observations.
"""
from __future__ import annotations

from collections import Counter
import json
from pathlib import Path
import sys
from typing import Any

BASE = Path(__file__).resolve().parent
SOURCE = 'ac02e7ccb7e2b9e842e5f8dd15903b14f7fff9b9426909449b36b33bbd08db6c'
BINARY = '557b2e4d0bc89f429a07a270c9f388b508d595c8b4a1f233b795becdf75797e7'
THRESHOLD = 3_328_599_654
POOL_BYTES = 67_108_864
R07_MAPPED_BYTES = 277_610_496
CHECKS: list[dict[str, Any]] = []
BLOCKERS: list[str] = []


def read_json(path: Path, limit: int = 32 * 1024 * 1024) -> Any:
    if not path.is_file():
        raise FileNotFoundError(path)
    size = path.stat().st_size
    if size > limit:
        raise RuntimeError(f'report exceeds {limit} bytes: {path}')
    return json.loads(path.read_text(encoding='utf-8'))


def check(name: str, passed: bool, detail: Any, *, blocking: bool = True) -> None:
    CHECKS.append({'name': name, 'passed': bool(passed), 'detail': detail,
                   'blocking': blocking})
    if not passed and blocking:
        BLOCKERS.append(name)


def reg_roles(root: Path) -> dict[str, dict[str, Any]]:
    path = root / 'registrations.jsonl'
    roles: dict[str, dict[str, Any]] = {}
    if not path.is_file():
        return roles
    for line in path.read_text(encoding='utf-8').splitlines():
        row = json.loads(line)
        if row.get('kind') != 'started':
            continue
        ident = (row.get('sample') or {}).get('identity') or {}
        roles[row.get('role')] = {
            'pid': ident.get('pid'), 'birth': ident.get('start_abstime'),
            'parent_pid': ident.get('parent_pid'),
        }
    return roles


def inspect_window(run_name: str) -> dict[str, Any]:
    root = BASE / run_name
    path = root / 'full-restored-window.json'
    obj = read_json(path)
    frames = obj.get('frames') or []
    roles = reg_roles(root)
    detail: dict[str, Any] = {
        'schema': obj.get('schema'), 'source_sha256': obj.get('source_sha256'),
        'frame_count': len(frames), 'times': [f.get('t') for f in frames],
        'frame_group_bytes': [], 'member_sum_bytes': [], 'frame_checks': [],
    }
    for frame in frames:
        applied = frame.get('r07_applied') or {}
        state = applied.get('state') or {}
        owner = applied.get('owner') or {}
        members = frame.get('members') or []
        root_rows = [r for r in members if len(r) == 5 and r[4] == 'r07-layout-driver']
        layout_rows = [r for r in members if len(r) == 5 and r[4] == 'layout-r07-session']
        root_id = roles.get('r07-layout-driver') or {}
        layout_id = roles.get('layout-r07-session') or {}
        footprints = frame.get('member_footprints_pid_birth_bytes') or []
        footprint_valid = all(isinstance(r, list) and len(r) == 3 and isinstance(r[2], int)
                              for r in footprints)
        member_sum = sum(r[2] for r in footprints) if footprint_valid else None
        group_bytes = frame.get('phys_footprint_bytes')
        exact_ids = bool(
            len(root_rows) == 1 and len(layout_rows) == 1
            and root_rows[0][0] == root_id.get('pid') and root_rows[0][2] == root_id.get('birth')
            and layout_rows[0][0] == layout_id.get('pid') and layout_rows[0][2] == layout_id.get('birth')
            and owner.get('pid') == layout_id.get('pid')
            and owner.get('start_abstime') == layout_id.get('birth')
        )
        predicate = {
            'group_capture_complete': frame.get('group_capture_complete') is True,
            'r07_coverage_complete': frame.get('r07_coverage_complete') is True,
            'provider_paused': frame.get('r07_provider_paused') is True,
            'source_matches': applied.get('source') == SOURCE,
            'mapped_full': state.get('mapped_bytes') == R07_MAPPED_BYTES,
            'replaced_zero': state.get('replaced_bytes') == 0,
            'permitted_zero': applied.get('permitted_bytes') == 0,
            'exact_root_layout_owner_pid_birth': exact_ids,
            'member_footprints_complete': footprint_valid and len(footprints) == len(members),
            'member_sum_equals_group': member_sum is not None and member_sum == group_bytes,
        }
        detail['frame_group_bytes'].append(group_bytes)
        detail['member_sum_bytes'].append(member_sum)
        detail['frame_checks'].append(predicate)
        detail.setdefault('condition_vectors', []).append({
            'source': applied.get('source'),
            'state_schema': state.get('schema'),
            'logical_bytes': state.get('logical_bytes'),
            'mapped_bytes': state.get('mapped_bytes'),
            'replaced_bytes': state.get('replaced_bytes'),
            'permitted_bytes': applied.get('permitted_bytes'),
            'group_capture_complete': frame.get('group_capture_complete'),
            'r07_coverage_complete': frame.get('r07_coverage_complete'),
            'provider_paused': frame.get('r07_provider_paused'),
        })

    times = detail['times']
    consecutive = (len(times) == 3 and all(isinstance(t, (int, float)) for t in times)
                   and all(0 < times[i + 1] - times[i] <= 1.0 for i in range(2)))
    all_predicates = (len(frames) == 3 and all(all(x.values()) for x in detail['frame_checks']))
    groups = [int(x) for x in detail['frame_group_bytes'] if isinstance(x, int)]
    detail['consecutive_within_1s'] = consecutive
    detail['all_restored_predicates'] = all_predicates
    detail['window_peak_bytes'] = max(groups) if len(groups) == 3 else None
    detail['roles'] = roles
    return detail


def event_rejections(root: Path) -> dict[str, int]:
    path = root / 'model' / 'normal-pool' / 'events.jsonl'
    counts: Counter[str] = Counter()
    if not path.is_file():
        return {}
    for line in path.read_text(encoding='utf-8').splitlines():
        if len(line) > 65536:
            raise RuntimeError(f'oversized event line: {path}')
        row = json.loads(line)
        if row.get('kind') == 'rejected':
            counts[row.get('rejection_reason', '<missing>')] += 1
    return dict(counts)


def terminal_audit(receipt: dict[str, Any]) -> dict[str, Any]:
    audit = receipt.get('identity_audit') or {}
    identities = audit.get('identities') or []
    states = Counter(x.get('status') for x in identities)
    all_terminal = bool(identities) and all(
        x.get('status') in ('confirmed_absent', 'pid_reused_new_birth') for x in identities)
    return {'complete': audit.get('complete') is True, 'identity_count': len(identities),
            'states': dict(states), 'all_terminal_by_receipt': all_terminal}


def has_pid_identity(value: Any, pid: int) -> bool:
    if isinstance(value, dict):
        if value.get('pid') == pid and ('start_abstime' in value or 'parent_pid' in value):
            return True
        return any(has_pid_identity(child, pid) for child in value.values())
    if isinstance(value, list):
        return any(has_pid_identity(child, pid) for child in value)
    return False


def controlled_run(run_name: str, *, require_excluded: bool | None) -> dict[str, Any]:
    root = BASE / run_name
    receipt = read_json(root / 'receipt.json')
    observer = read_json(root / 'observer-report.json')
    manifest = read_json(root / 'model' / 'manifest.json')
    exercise = read_json(root / 'model' / 'exercise-report.json')
    layout_report = read_json(root / 'model' / 'layout' / 'report.json')
    cold_report = read_json(root / 'model' / 'cold-with-provider' / 'report.json')
    pool_report = read_json(root / 'model' / 'normal-pool' / 'report.json')
    window = inspect_window(run_name)
    roles = reg_roles(root)
    terminal = terminal_audit(receipt)
    source_ok = (manifest.get('source_sha256') == SOURCE
                 and exercise.get('source_sha256') == SOURCE
                 and observer.get('source_sha256') == SOURCE
                 and receipt.get('source_sha256') == SOURCE
                 and receipt.get('source_sha256_matches_fixed_binary') is True
                 and receipt.get('binary_sha256') == BINARY
                 and observer.get('binary_sha256') == BINARY)
    pool_summary = pool_report.get('summary') or {}
    snapshots = layout_report.get('snapshots') or []
    cold_restore = cold_report.get('restore') or {}
    details = {
        'source_and_binary_match_fixed': source_ok,
        'observer': {k: observer.get(k) for k in (
            'status', 'model_exit_code', 'samples', 'sample_log_bytes',
            'missing_footprint_process_samples', 'maximum_sample_gap_seconds',
            'maximum_observed_r07_replaced_bytes', 'peak_simultaneous_footprint_bytes',
            'unpolled_registered_processes', 'full_M0', 'eight_hour_gate')},
        'window': window,
        'snapshots': {'schema': layout_report.get('schema'),
                      'generations': [x.get('generation') for x in snapshots],
                      'complete_record_restore': [x.get('complete_record_restore') for x in snapshots],
                      'formal_steady': layout_report.get('formal_steady'),
                      'eight_hour': layout_report.get('eight_hour')},
        'cold_report': {'schema': cold_report.get('schema'),
                        'cold_reopen_and_scan': cold_report.get('cold_reopen_and_scan'),
                        'same_source': cold_restore.get('same_source'),
                        'producer_source_sha256': cold_restore.get('producer_source_sha256'),
                        'reader_source_sha256': cold_restore.get('reader_source_sha256'),
                        'all_payload_files_hashed_and_loaded': cold_restore.get('all_payload_files_hashed_and_loaded'),
                        'restored_generation': cold_restore.get('restored_generation'),
                        'byte_length': cold_restore.get('byte_length'),
                        'payload_hashes_independently_rechecked': False},
        'normal_pool': {'capacity_bytes': pool_summary.get('capacity_bytes'),
                        'available_touched_bytes': pool_summary.get('available_touched_bytes'),
                        'granted_capacity_bytes': pool_summary.get('granted_capacity_bytes'),
                        'grants': pool_summary.get('grants'),
                        'workspace_grants': pool_summary.get('workspace_grants'),
                        'balanced_after_owned_wait': pool_report.get('balanced_after_owned_wait')},
        'terminal_identity_audit': terminal,
        'registration_roles': roles,
        'controlled_receipt': {'passed_controlled_exercise': receipt.get('passed_controlled_exercise'),
                               'passed_fault_exercise': receipt.get('passed_fault_exercise'),
                               'nested_outcome_passed': (receipt.get('outcome_checks') or {}).get('passed'),
                               'nested_checks': (receipt.get('outcome_checks') or {}).get('checks')},
    }
    reasons: dict[str, Any] = {}
    if require_excluded is True:
        xpath = root / 'r07-excluded-root.json'
        x = read_json(xpath) if xpath.exists() else {}
        identity = x.get('identity') or {}
        root_role = roles.get('r07-layout-driver') or {}
        runtime = read_json(root / 'r07-runtime.json')
        exclusion_ok = (x.get('schema') == 'mycity-r07-excluded-mixed-root-v1'
                        and x.get('source_sha256') == SOURCE
                        and x.get('exclusion') == 'entire_actual_child_pid_for_lifetime'
                        and x.get('estimated_footprint_deduction_bytes') == 0
                        and x.get('raw_group_unchanged') is True
                        and identity.get('pid') == root_role.get('pid')
                        and identity.get('start_abstime') == root_role.get('birth')
                        and identity.get('parent_pid') == root_role.get('parent_pid'))
        # The runtime's final serialized components must not include the excluded
        # root identity. The explicit exclusion receipt is the lifetime proof.
        runtime_does_not_list_root = not has_pid_identity(runtime.get('components'), root_role.get('pid'))
        reasons['combined_root_exclusion'] = {
            'schema': x.get('schema'), 'root_identity': identity,
            'exclusion': x.get('exclusion'),
            'estimated_footprint_deduction_bytes': x.get('estimated_footprint_deduction_bytes'),
            'raw_group_unchanged': x.get('raw_group_unchanged'),
            'runtime_does_not_list_root_pid': runtime_does_not_list_root,
            'passes': exclusion_ok and runtime_does_not_list_root,
        }
    elif require_excluded is False:
        x_exists = (root / 'r07-excluded-root.json').exists()
        roles_ok = ('normal-pool-host' in roles
                    and roles['normal-pool-host'].get('pid') != roles.get('r07-layout-driver', {}).get('pid'))
        reasons['separate_pool_not_excluded'] = {
            'mixed_root_exclusion_receipt_absent': not x_exists,
            'independent_normal_pool_host_registered': roles_ok,
            'root_in_complete_physical_frames': all(
                any(len(m) == 5 and m[4] == 'r07-layout-driver'
                    and m[0] == roles.get('r07-layout-driver', {}).get('pid')
                    and m[2] == roles.get('r07-layout-driver', {}).get('birth')
                    for m in f.get('members', [])) for f in (read_json(root / 'full-restored-window.json').get('frames') or [])),
            'passes': not x_exists and roles_ok,
        }
    details['r07_exclusion'] = reasons

    gen_ok = ([x.get('generation') for x in snapshots] == [1, 2]
              and all(x.get('complete_record_restore') == 'passed'
                      and x.get('full_pressure_file_readback') == 'passed' for x in snapshots))
    payload_report_ok = (cold_report.get('schema') == 'mycity-layout-cold-reopen-v2'
                         and cold_report.get('cold_reopen_and_scan') == 'passed'
                         and cold_restore.get('same_source') is True
                         and cold_restore.get('producer_source_sha256') == SOURCE
                         and cold_restore.get('reader_source_sha256') == SOURCE
                         and cold_restore.get('all_payload_files_hashed_and_loaded') is True
                         and cold_restore.get('restored_generation') == 2)
    pool_ok = (pool_summary.get('capacity_bytes') == POOL_BYTES
               and pool_summary.get('available_touched_bytes') == POOL_BYTES
               and pool_summary.get('granted_capacity_bytes') == 0
               and pool_summary.get('grants') == 0
               and pool_summary.get('workspace_grants') == 0
               and pool_report.get('balanced_after_owned_wait') is True)
    window_ok = (window.get('schema') == 'mycity-r07-full-restored-window-v1'
                 and window.get('source_sha256') == SOURCE
                 and window.get('frame_count') == 3
                 and window.get('consecutive_within_1s') is True
                 and window.get('all_restored_predicates') is True)
    check(f'{run_name}:fixed-source-and-binary', source_ok, {'source': SOURCE, 'binary': BINARY})
    check(f'{run_name}:full-restored-three-frame-window', window_ok, window)
    check(f'{run_name}:two-snapshots-and-cold-report', gen_ok and payload_report_ok,
          {'generations': [x.get('generation') for x in snapshots], 'cold_report': details['cold_report']})
    check(f'{run_name}:normal-pool-64MiB-zero-grants', pool_ok, details['normal_pool'])
    check(f'{run_name}:all-registered-identities-terminal', terminal['complete'] and terminal['all_terminal_by_receipt'], terminal)
    if require_excluded is not None:
        key = 'combined_root_exclusion' if require_excluded else 'separate_pool_not_excluded'
        check(f'{run_name}:{key}', reasons[key]['passes'], reasons[key])
    details['checks_pass'] = all(c['passed'] for c in CHECKS if c['name'].startswith(run_name + ':'))
    return details


def main() -> int:
    final = read_json(BASE / 'binary-final.json')
    old = read_json(BASE / 'binary.json')
    fixed_identity = {
        'source_sha256': final.get('source_sha256'), 'binary_sha256': final.get('binary_sha256'),
        'source_zip_sha256': final.get('source_zip_sha256'),
        'original_normal_threshold_bytes': final.get('original_normal_threshold_bytes'),
        'normal_pool_bytes': final.get('normal_pool_bytes'),
        'full_r07_mapped_bytes': final.get('full_r07_mapped_bytes'),
    }
    check('binary-final-fixed-identity',
          fixed_identity['source_sha256'] == SOURCE and fixed_identity['binary_sha256'] == BINARY
          and fixed_identity['original_normal_threshold_bytes'] == THRESHOLD
          and fixed_identity['normal_pool_bytes'] == POOL_BYTES
          and fixed_identity['full_r07_mapped_bytes'] == R07_MAPPED_BYTES,
          fixed_identity)
    check('legacy-binary-json-excluded-from-comparison',
          old.get('binary_sha256') != BINARY and old.get('source_sha256') is None,
          {'legacy_binary_sha256': old.get('binary_sha256'),
           'legacy_source_sha256_in_record': old.get('source_sha256'),
           'context_source_prefix': '4a818 (legacy R07-attribution runs excluded)'})

    results: dict[str, Any] = {}
    for name, exclusion in (('separate-control', False), ('combined-control', True), ('half-wire03', True)):
        results[name] = controlled_run(name, require_excluded=exclusion)

    # The separate-control receipt intentionally retains its helper's old nested
    # half-wire verdict. Check the real control checks/top-level result separately.
    separate_receipt = read_json(BASE / 'separate-control' / 'receipt.json')
    nested = separate_receipt.get('outcome_checks') or {}
    nested_checks = nested.get('checks') or {}
    discrepancy = {
        'passed_controlled_exercise': separate_receipt.get('passed_controlled_exercise'),
        'nested_outcome_passed': nested.get('passed'),
        'all_nested_checks_true': bool(nested_checks) and all(nested_checks.values()),
        'known_reason': 'retained helper reused half-wire outcome rule; do not rewrite historical receipt',
    }
    check('separate-control-receipt-discrepancy-recorded',
          discrepancy['passed_controlled_exercise'] is True
          and discrepancy['nested_outcome_passed'] is False
          and discrepancy['all_nested_checks_true'], discrepancy, blocking=False)
    results['separate_control_receipt_discrepancy'] = discrepancy

    # Controlled negative fault tests: exact intended target was signaled and the
    # observer failed with a non-success model result, with all identities retired.
    for name, expected_signal in (('kill-layout', 'SIGTERM'), ('kill-root', 'SIGKILL')):
        root = BASE / name
        receipt = read_json(root / 'receipt.json')
        observer = read_json(root / 'observer-report.json')
        target = read_json(root / 'signal-intent.json')
        proof = read_json(root / 'fault-target-proof.json')
        terminal = terminal_audit(receipt)
        okay = (receipt.get('passed_fault_exercise') is True
                and receipt.get('source_sha256') == SOURCE
                and receipt.get('binary_sha256') == BINARY
                and target.get('signal') == expected_signal
                and proof.get('target_identity_check', {}).get('ok') is True
                and target.get('pid') == proof.get('target_identity_check', {}).get('pid')
                and target.get('birth') == proof.get('target_identity_check', {}).get('expected_birth')
                and observer.get('status') == 'failed'
                and observer.get('model_exit_code') != 0
                and terminal['complete'] and terminal['all_terminal_by_receipt'])
        details = {'signal': target.get('signal'), 'target_pid': target.get('pid'),
                   'target_birth': target.get('birth'),
                   'target_identity_check_ok': proof.get('target_identity_check', {}).get('ok'),
                   'observer_status': observer.get('status'),
                   'model_exit_code': observer.get('model_exit_code'), 'terminal_audit': terminal}
        check(f'{name}:identity-bound-negative-fault', okay, details)
        results[name] = details

    half03 = read_json(BASE / 'half-wire03' / 'receipt.json')
    half03_events = event_rejections(BASE / 'half-wire03')
    half03_ok = (half03.get('passed_fault_exercise') is True
                 and (half03.get('outcome_checks') or {}).get('passed') is True
                 and half03_events == {'normal_pool_control_timeout': 1})
    check('half-wire03:one-timeout-rejection-and-successful-cold-path', half03_ok,
          {'fault_exercise_passed': half03.get('passed_fault_exercise'),
           'outcome_passed': (half03.get('outcome_checks') or {}).get('passed'),
           'rejections': half03_events})
    results['half-wire03_rejections'] = half03_events

    half02_events = event_rejections(BASE / 'half-wire02')
    check('half-wire02-legacy-rejection-distinction',
          half02_events == {'normal_pool_control_eof': 47, 'normal_pool_control_timeout': 1},
          {'rejections': half02_events,
           'interpretation': '47 EOF rejections from repeated/closed requests and one explicit incomplete-frame timeout; distinct outcomes'})
    results['half-wire02_rejections'] = half02_events

    # Retain stage 10 and 15 failed execution records as history; no cleanup.
    failures: dict[str, Any] = {}
    for stage in ('10-half-wire', '15-separate-control'):
        row = read_json(BASE / stage / 'exit.json')
        failures[stage] = row
        check(f'{stage}:failure-artifact-retained', row.get('exit_code') == 1
              and row.get('source_unchanged') is True, row)
    results['retained_failure_stages'] = failures

    # Cold-error is a deliberately failed cold-open attempt, not a performance pass.
    cold_root = BASE / 'cold-error'
    cold_receipt = read_json(cold_root / 'receipt.json')
    cold_observer = read_json(cold_root / 'observer-report.json')
    cold_error = (cold_receipt.get('outcome_checks') or {}).get('cold_failure_output', '')
    cold_audit = terminal_audit(cold_receipt)
    cold_info = {
        'model_exit_code': cold_observer.get('model_exit_code'),
        'observer_status': cold_observer.get('status'),
        'cold_child_registered': (cold_receipt.get('outcome_checks') or {}).get('cold_child_actually_registered'),
        'already_exists_error': 'AlreadyExists' in cold_error,
        'successful_exercise': not (cold_receipt.get('outcome_checks') or {}).get('no_successful_exercise'),
        'all_registered_identities_terminal': cold_audit,
        'unpolled_registered_processes': cold_observer.get('unpolled_registered_processes'),
        'full_M0': cold_observer.get('full_M0'),
    }
    cold_ok = (cold_receipt.get('passed_controlled_exercise') is True
               and cold_observer.get('model_exit_code') == 2
               and cold_info['cold_child_registered'] is True
               and cold_info['already_exists_error']
               and (cold_receipt.get('outcome_checks') or {}).get('no_successful_exercise') is True
               and cold_audit['complete'] and cold_audit['all_terminal_by_receipt']
               and cold_observer.get('unpolled_registered_processes') == 1
               and cold_observer.get('full_M0') == 'not_accepted')
    check('cold-error-retained-as-nonperformance-terminal-failure', cold_ok, cold_info)
    results['cold_error'] = cold_info

    # Compare only the exact three-frame full-restored windows; the global peak
    # is not comparable because the observer reports missing process samples.
    sep = results['separate-control']['window']
    comb = results['combined-control']['window']
    sep_peak = sep.get('window_peak_bytes')
    comb_peak = comb.get('window_peak_bytes')
    comparison_valid = (sep.get('all_restored_predicates') and comb.get('all_restored_predicates')
                        and sep.get('source_sha256') == SOURCE and comb.get('source_sha256') == SOURCE
                        and sep.get('frame_count') == comb.get('frame_count') == 3
                        and sep.get('condition_vectors') == comb.get('condition_vectors'))
    comparison = {
        'source_sha256': SOURCE, 'binary_sha256': BINARY,
        'comparison_basis': 'maximum phys_footprint_bytes among each run exact full-restored three-frame window; not observer global peak',
        'separate_window_peak_bytes': sep_peak,
        'combined_window_peak_bytes': comb_peak,
        'separate_minus_combined_bytes': (sep_peak - comb_peak) if comparison_valid else None,
        'original_normal_threshold_bytes': THRESHOLD,
        'separate_over_threshold_bytes': (sep_peak - THRESHOLD) if comparison_valid else None,
        'combined_over_threshold_bytes': (comb_peak - THRESHOLD) if comparison_valid else None,
        'both_same_source_and_predicates': comparison_valid,
        'observer_global_peaks_with_missing_samples_not_used': {
            'separate': results['separate-control']['observer'].get('peak_simultaneous_footprint_bytes'),
            'combined': results['combined-control']['observer'].get('peak_simultaneous_footprint_bytes'),
            'separate_missing_samples': results['separate-control']['observer'].get('missing_footprint_process_samples'),
            'combined_missing_samples': results['combined-control']['observer'].get('missing_footprint_process_samples'),
        },
    }
    comparison_ok = bool(comparison_valid and sep_peak == 3_337_555_520
                         and comb_peak == 3_336_228_080
                         and comparison['separate_minus_combined_bytes'] == 1_327_440
                         and comparison['separate_over_threshold_bytes'] == 8_955_866
                         and comparison['combined_over_threshold_bytes'] == 7_628_426)
    check('same-source-full-restored-window-peak-comparison', comparison_ok, comparison)
    results['peak_comparison'] = comparison

    output = {
        'schema': 'mycity-pool-control-final-report-check-v1',
        'read_only': True,
        'snapshot_payload_files_opened_or_rehashed': False,
        'performance_or_formal_acceptance_claimed': False,
        'fixed_identity': fixed_identity,
        'results': results,
        'checks': CHECKS,
        'blockers': BLOCKERS,
        'verification_passed': not BLOCKERS,
        'interpretation': {
            'separate_control': 'controlled exercise true; retained nested verdict inconsistency is explicitly reported',
            'combined_control': 'controlled exercise and helper checks pass; excluded mixed-root deduction is zero and raw group remains intact',
            'half_wire03': 'single timeout rejection and full snapshot/cold report checks pass',
            'kill_cases': 'intended identity-bound negative cases pass; not functional/performance acceptance',
            'cold_error': 'terminal AlreadyExists failure; not performance acceptance',
            'formal': 'not run/not accepted',
        },
    }
    print(json.dumps(output, indent=2, sort_keys=True))
    return 0 if not BLOCKERS else 1


if __name__ == '__main__':
    raise SystemExit(main())
