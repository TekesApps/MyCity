"""Offline conservative process-group capture selection; not leakage acceptance."""
import json,sys
from pathlib import Path
D=Path(sys.argv[1]);load=lambda p:json.loads(p.read_text());M=D/'model';r=load(M/'business/report.json');o=load(D/'observer-report.json');s=load(D/'observer-clock-map.json');d=load(M/'clock-map.json')
lo=s['supervisor_origin_observer_ns_min']+d['driver_origin_parent_ns_min'];hi=s['supervisor_origin_observer_ns_max']+d['driver_origin_parent_ns_max'];assert lo<=hi
start=round(r['warmup_seconds']*1e9);end=round(r['fixed_window_end_seconds']*1e9)
inside=outside=uncertain=0;rss=foot=missing=0;first=last=None;gap=0
for line in (D/'host-samples.jsonl').open():
 f=json.loads(line);a=f['clock_begin_ns'];b=f['clock_end_ns'];assert a<=b
 if a-hi>=start and b-lo<=end:
  inside+=1;rss=max(rss,f['rss_bytes']);foot=max(foot,f['phys_footprint_bytes']);missing+=f['missing_footprint']
  if first is None:first=b
  if last is not None:gap=max(gap,b-last)
  last=b
 elif b-lo<=start or a-hi>=end:outside+=1
 else:uncertain+=1
assert inside+outside+uncertain==o['samples']
print(json.dumps(dict(schema='mycity-steady-resource-window-v1',source_sha256=r['source_sha256'],formal=r['formal'],driver_origin_observer_ns=[lo,hi],mapping_uncertainty_ns=hi-lo,inside_capture_frames=inside,outside_capture_frames=outside,crossing_or_uncertain_capture_frames=uncertain,inside_sampled_simultaneous_rss_peak_bytes=rss,inside_obtained_simultaneous_footprint_peak_bytes=foot,inside_missing_footprint_process_samples=missing,inside_maximum_capture_end_gap_ns=gap,whole_observation_status=o['status'],whole_maximum_swap_net_growth_bytes=o['maximum_observed_swap_net_growth_bytes'],whole_maximum_non_normal_pressure_seconds=o['maximum_sampled_non_normal_pressure_seconds'],whole_peak_world_logical_bytes=o['peak_world_logical_bytes'],whole_observer_hot_bound_bytes=o['observer_hot_bound_bytes'],whole_combined_reported_diagnostic_hot_bound_bytes=o['combined_diagnostic_hot_bound_bytes'],scope='Whole-group captures only; missing processes/footprint never imputed. Mapping uses all causal uncertainty, no interpolation.',normal_3_10GiB='not_accepted; complete ownership/resource closure separately required',shared_8MiB='not_accepted; role and runtime peak coupling separately required',same_boundary_leakage='not_assessed; day Status marker alone is not a coincident resource sample or explanation of live history',three_rounds='not_accepted_by_single_round',eight_hour='not_run',real_16GB='not_run',full_M0='not_accepted'),indent=2))
