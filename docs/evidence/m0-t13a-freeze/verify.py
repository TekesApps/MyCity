#!/usr/bin/env python3
"""Verify T13A-0 frozen workload/acceptance only; no runtime gate is granted."""
import hashlib
import json
from pathlib import Path
import re
import subprocess

HERE = Path(__file__).resolve().parent
ROOT = HERE.parents[2]


def require(ok, why):
    if not ok:
        raise SystemExit(why)


def digest(raw):
    return hashlib.sha256(raw).hexdigest()


def main():
    raw = (ROOT / 'fixtures/m0-t13a-acceptance.json').read_bytes()
    p = json.loads(raw)
    ref = json.loads((ROOT / 'docs/inputs/product-blueprint/reference-values.json').read_bytes())
    require(p['l1']['counts'] == ref['jprod_1'] and p['l1']['app_gib'] == ref['app_gib'], 'JPROD scope or budget reduced')
    require(sum(p['l1']['app_gib'].values()) == 3.1, 'domain budget')
    require(p['steady'] == {'scenarios': ['L0_pack', 'L1_jprod_layout', 'L2_full_joint'],
            'independent_runs_each': 3, 'warmup_wall_seconds': 300, 'measured_wall_seconds': 1800,
            'tier_diagnostics_are_not_steady_acceptance': True}, 'steady gate reduced')
    require(p['endurance']['minimum_wall_seconds'] == 28800 and p['endurance']['game_days'] == 30
            and p['endurance']['game_hour_wall_seconds'] == 40 and p['endurance']['autosave_requests'] == 96
            and p['endurance']['same_persistent_world'], 'endurance scope')
    require(30 * 24 * 40 == 28800 and 28800 // 300 == 96, 'pace/save arithmetic')
    require(p['latency']['batch_compute_p95_us'] == 20000 and p['latency']['batch_compute_p99_us'] == 40000
            and p['latency']['minimum_class_samples_for_quantiles'] == 200
            and len(p['latency']['compute_classes']) == 10, 'latency sample gate')
    require(p['speed']['day_wall_seconds'] == [600, 60, 10, 1]
            and p['speed']['mandatory_pe0_actual_operating_30days_max_wall_seconds'] == 1800, 'operating throughput gate')
    memory = p['memory']
    require([memory[k] for k in ['normal_gib', 'soft_gib', 'peak_guard_gib']] == [3.1, 3.5, 4]
            and memory['diagnostic_hot_bytes'] == 8 * 1024**2
            and memory['swap_net_growth_max_bytes'] == 256 * 1024**2, 'memory gates')
    require(p['disk']['world_max_bytes'] == 20 * 1024**3 and p['disk']['minimum_free_bytes'] == 2 * 1024**3, 'disk gates')
    full = p['l2_tiers'][-1]
    require([full[k] for k in ['accounts', 'stocks', 'payroll', 'claims']] == [64, 128, 64, 20000], 'full engine scope')
    files = {'crates/city-core/src/assembly.rs': {'MAX_ACCOUNTS': 64, 'MAX_STOCKS': 128},
             'crates/city-core/src/timeline.rs': {'PAYROLL_LIMIT': 64, 'CLAIM_LIMIT': 20000},
             'crates/city-core/src/transaction.rs': {'MAX_RESOURCES': 8, 'MAX_LEASES': 64}}
    for name, constants in files.items():
        baseline = subprocess.check_output(['git', 'show', 'v0.5.29:' + name], cwd=ROOT, text=True)
        for key, value in constants.items():
            require(re.search(r'pub const ' + key + r': usize = ' + str(value) + r';', baseline), 'baseline cap differs: ' + key)
    probe = json.loads((HERE / 'footprint-feasibility.json').read_bytes())
    require(probe['scope'] == 'instrumentation_feasibility_only' and probe['no_model_gate_claimed'], 'instrumentation scope')
    require(len(probe['rows']) == 2 and all(row['phys_footprint_bytes'] > 0 for row in probe['rows']), 'physical footprint unavailable')
    calibration = probe['cpu_clock_calibration']
    require(abs(calibration['converted_ns'] - calibration['process_time_ns_delta']) < 10_000_000, 'CPU clock units unresolved')
    require((HERE / 'advisor-review.md').is_file(), 'independent plan review missing')
    result = {'schema': 'mycity-t13a-freeze-check-v1', 'acceptance_sha256': digest(raw),
              'baseline_tag': 'v0.5.29',
              'baseline_commit': subprocess.check_output(['git', 'rev-parse', 'v0.5.29^{}'], cwd=ROOT, text=True).strip(),
              'workload_and_acceptance_frozen': True, 'advisor_plan_review': 'completed_read_only',
              'instrumentation_feasibility': 'passed_only_for_named_probe',
              'steady_minimum_seconds_excluding_diagnostics': 3 * 3 * (300 + 1800),
              'endurance_minimum_seconds': 28800, 'model_scale_gate': 'not_run',
              'steady_gate': 'not_run', 'eight_hour_gate': 'not_run', 'memory_16gb_gate': 'not_run',
              'full_M0': 'not_accepted', 'verifier_sha256': digest(Path(__file__).read_bytes())}
    (HERE / 'checks.json').write_text(json.dumps(result, indent=2) + '\n')
    print(json.dumps(result, indent=2))


if __name__ == '__main__':
    main()
