"""Summarize actual single L1 run, preserving failed/unmeasured gates."""
import hashlib
import json
import zipfile
from pathlib import Path

D = Path(__file__).resolve().parent
R = D.parents[2]
W = R/'work/typed-workspace-formal-l1-01/observed'
load = lambda p: json.loads(p.read_text())
info = load(D/'source.json')
with zipfile.ZipFile(D/'frozen-source.zip') as archive:
    for name in archive.namelist():
        assert (R/name).read_bytes() == archive.read(name), name
assert hashlib.sha256(Path(info['binary']).read_bytes()).hexdigest() == info['binary_sha256']
observer = load(W/'observer-report.json')
layout = load(W/'model/layout/report.json')
business = load(W/'model/business/report.json')
latency = load(D/'window_audit.json')
resources = load(D/'resource_window_audit.json')
residency = load(D/'layout_residency_audit.json')
assert observer['model_exit_code'] == 0 and layout['diagnostic_outcome'] == {'Ok':None}
assert business['scenario'] == 'l1' and business['warmup_seconds'] == 300
assert business['measurement_seconds'] == 1800 and business['fixed_window_end_seconds'] == 2100
assert layout['source_sha256'] == latency['source_sha256'] == info['source_sha256']
events = [json.loads(line) for line in (W/'model/layout/events.jsonl').open()]
scans = [row for row in events if 'scan' in row]
durable = [row['snapshot_durable'] for row in events if 'snapshot_durable' in row]
assert len(scans) == layout['scans'] == layout['expected_published_scans'] == 2101
assert len(durable) == 8 and [row['generation'] for row in durable] == list(range(1,9))
assert all(row['complete_record_restore'] == row['full_pressure_file_readback'] == 'passed' for row in durable)
assert all(row['scan']['native_records'] == 361272 and row['scan']['stub_records'] == 1232426 for row in scans)
assert all(row['allocator_after_scan']['allocation_failures'] == 0 for row in scans)
idle = [row for row in residency['same_phase_samples'] if row['eligible_complete_frames']]
assert idle
first,last = idle[0]['allocator'],idle[-1]['allocator']
origin = resources['driver_origin_observer_ns']
window = latency['measurement_driver_ns']
complete = incomplete = 0
complete_rss = complete_footprint = 0
incomplete_examples = []
for line in (W/'host-samples.jsonl').open():
    frame = json.loads(line)
    if frame['clock_begin_ns']-origin[1] >= window[0] and frame['clock_end_ns']-origin[0] <= window[1]:
        if frame['group_capture_complete']:
            complete += 1
            complete_rss = max(complete_rss,frame['rss_bytes'])
            complete_footprint = max(complete_footprint,frame['phys_footprint_bytes'])
        else:
            incomplete += 1
            if len(incomplete_examples) < 3:
                incomplete_examples.append({key:frame.get(key) for key in
                    ('t','pending_identities','missing_selected_processes','identity_uncertainties','enumeration_failures','missing_footprint','missing_rss')})
assert complete+incomplete == resources['inside_capture_frames']
normal = 310*1024**3//100
footprint = resources['inside_obtained_simultaneous_footprint_peak_bytes']
rss = resources['inside_sampled_simultaneous_rss_peak_bytes']
assert footprint > normal and rss > normal
assert complete_footprint > normal and complete_rss > normal
eligible = [row for row in latency['strata'] if row['record_state']=='sealed' and row['role']=='execution' and row['window']=='inside']
current = [load(D/'cold-current7/report.json'),load(D/'cold-current8/report.json')]
assert all(row['cold_reopen_and_scan']=='passed' and row['restore']['all_payload_files_hashed_and_loaded'] for row in current)
result = dict(
    source_sha256=info['source_sha256'],binary_sha256=info['binary_sha256'],source_unchanged=True,
    actual_observer_seconds=observer['actual_wall_seconds'],actual_supervisor_seconds=load(W/'model/report.json')['whole_seconds'],
    actual_warmup_seconds=300,actual_measurement_seconds=1800,model_exit_code=0,
    evidence_class='L1 storage-pressure substitute; NOT L2 business, traffic or complete-city scale',
    scan_count=len(scans),native_records_per_scan=361272,stub_records_per_scan=1232426,
    maximum_full_scan_wall_seconds=max(row['scan']['wall_seconds'] for row in scans),
    maximum_scan_ready_age_seconds=layout['maximum_scan_ready_age_seconds'],
    complete_snapshots=len(durable),snapshot_durations_seconds=[row['durable_complete_seconds'] for row in durable],
    actual_cow_backings_returned=[row['cow_backings_returned_after_join'] for row in durable],
    allocation_failures=0,original_raw_universe_fully_accounted=latency['raw_universe_fully_accounted'],
    eligible_window_classes=eligible,observed_single_round_compute_thresholds_met=latency['observed_window_thresholds_met'],
    inside_complete_capture_frames=complete,inside_incomplete_capture_frames=incomplete,
    incomplete_capture_examples=incomplete_examples,
    complete_inside_group_rss_peak_bytes=complete_rss,complete_inside_group_footprint_peak_bytes=complete_footprint,
    whole_missing_samples=observer['missing_footprint_process_samples'],missing_samples_never_imputed=True,
    normal_cap_bytes=normal,inside_group_footprint_peak_bytes=footprint,
    inside_group_rss_peak_bytes=rss,footprint_excess_bytes=footprint-normal,rss_excess_bytes=rss-normal,
    normal_physical_gate='failed_observed; observer not subtracted; 16MiB drift tolerance not added to cap',
    diagnostic_hot_bound_bytes=observer['combined_diagnostic_hot_bound_bytes'],diagnostic_cap_bytes=8*1024**2,
    swap_net_growth_bytes=observer['maximum_observed_swap_net_growth_bytes'],
    non_normal_pressure_seconds=observer['maximum_sampled_non_normal_pressure_seconds'],
    world_peak_logical_bytes=observer['peak_world_logical_bytes'],
    same_phase_idle_gaps=len(idle),same_phase_complete_frames=sum(row['eligible_complete_frames'] for row in idle),
    idle_current_requested_delta_bytes=last['current_requested_bytes']-first['current_requested_bytes'],
    idle_arena_used_delta_bytes=last['arena_used_bytes']-first['arena_used_bytes'],
    idle_bootstrap_quarantined_delta_bytes=last['bootstrap_quarantined_bytes']-first['bootstrap_quarantined_bytes'],
    leakage_gate='not_accepted; L1 idle counters only, no L2 business day-boundary evidence',
    current_complete_restore_points=[str((W/'model/layout/snapshots'/f'snapshot-{g:06}').relative_to(R)) for g in (7,8)],
    earlier_current_run_generations='1..6 retired by two-generation policy; not directly restorable',
    current_two_full_cold_restores='passed; full byte input, no reconstructed replacement',
    audit_scope_correction='quiescent_audit attempt failed because L1 has no L2 business transcript; original stderr retained, not counted as pass',
    same_source_three_rounds='not_complete',four_speeds='not_run_this_source',
    eight_hour='not_run',real16gb='not_run',full_M0='not_accepted',
)
(D/'formal-result.json').write_text(json.dumps(result,indent=2)+'\n')
print(json.dumps({k:v for k,v in result.items() if k not in ('eligible_window_classes','current_complete_restore_points')},indent=2))
