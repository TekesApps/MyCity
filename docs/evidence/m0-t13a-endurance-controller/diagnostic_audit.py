#!/usr/bin/env python3
"""Measure this controller's component evidence without accepting absent/joint budgets."""
import collections,json,sys
from pathlib import Path
R=Path(sys.argv[1]);M=R/'model';B=M/'business';load=lambda p:json.loads(p.read_text());MiB=1024**2
heaps=[json.loads(s) for s in (M/'heaps.jsonl').read_text().splitlines()]
INFO=Path(sys.argv[2]) if len(sys.argv)>2 else Path(__file__).resolve().parent;info=load(INFO/'source.json');source=info['source_sha256'];assert all(h['source_sha256']==source for h in heaps)
reports=[load(p) for p in (B/'diagnostics').glob('*/report.json')]
normal={};caps={'sim':5*MiB//2,'worker':MiB}
for role in ['sim','worker']:
 xs=[r for r in reports if r['producer']==role];assert xs
 for x in xs:
  assert x['source_sha256']==source and x['charged_diagnostics_bound_bytes']==x['owned_diagnostics_bytes']+x['transient_reservation_bytes']
  assert x['charged_diagnostics_bound_bytes']<=caps[role]
 normal[role]={'reports':len(xs),'maximum_reported_component_charge_bytes':max(x['charged_diagnostics_bound_bytes'] for x in xs),'component_cap_bytes':caps[role],'scope':'sealed component capacity evidence; not whole process or missing-process proof'}
driver=[h for h in heaps if h['role']=='scale-endurance'];assert len(driver)==1
peak=lambda h:h['heap']['lifetime_peak_requested_bytes']+h['tail_reserved_bytes']
assert peak(driver[0])<=MiB
normal['driver']={'whole_Rust_heap_peak_plus_tail_bytes':peak(driver[0]),'cap_bytes':MiB,'native_stack_foreign':'separate'}
helper=[]
for mode in ['reference','fault-reference','before-reopen','compare-endurance']:
 x=load(B/mode/'result.json');c=x['diagnostic_capacity'];assert c['limit_bytes']==MiB//4
 assert sum(c[k] for k in ['control_slots_bytes','parser_error_scratch_bytes','paths_and_metadata_bytes','golden_or_hash_bytes','exit_tail_bytes'])==c['limit_bytes']
 h=next(h for h in heaps if (h['pid'],h['instance_unix_ns'])==(x['pid'],x['instance_unix_ns']))
 helper.append({'mode':mode,'control_capacity':c,'whole_Rust_heap_peak_plus_tail_bytes_including_model_work':peak(h),'scope':'static diagnostic owners separately counted; whole model peak is EXTRA, never deducted from pool'})
normal['helper']=helper
pool=next(h for h in heaps if h['role']=='normal-pool-host');assert peak(pool)<=64*MiB+MiB//4
normal['pool']={'whole_Rust_heap_peak_plus_tail_bytes':peak(pool),'whole_upper_bound_bytes':64*MiB+MiB//4,'diagnostic_0_25MiB_separate_proof':'not_closed; no subtraction of lifetime peaks while pages are lent'}
observer=load(R/'observer-report.json');runner=load(M/'runner-report.json')
assert observer['observer_hot_bound_bytes']<=2*MiB and runner['hot_trace']['charged_hot_bytes']<=MiB
normal['observer']={'observed_hot_trace_bound_bytes':observer['observer_hot_bound_bytes'],'cap_bytes':2*MiB,'startup_native_foreign':'not_proven_by_hot_tracemalloc'}
normal['Python_runner']={'observed_hot_trace_bound_bytes':runner['hot_trace']['charged_hot_bytes'],'cap_bytes':MiB,'startup_interpreter_native':'unclosed; replace formal runner with Rust supervisor'}
missing=[]
for p in (B/'diagnostics').iterdir():
 if not p.is_dir():continue
 pid=int(p.name.split('-')[1])
 if not any(h['pid']==pid for h in heaps):missing.append({'producer':p.name,'pid':pid,'sealed_component_report_present':(p/'report.json').exists(),'normal_heap_footer':'absent; not zero'})
waits=[json.loads(s) for s in (M/'normal-pool/events.jsonl').read_text().splitlines()]
failed=[r for r in waits if r['kind']=='owned_child_wait_failure']
assert failed[0]['parent'] in {m['pid'] for m in missing}
assert {m['pid'] for m in missing}<={r['pid'] for r in failed}|{failed[0]['parent']}
assert collections.Counter(p.name.split('-')[0] for p in (B/'diagnostics').iterdir() if p.is_dir() and not (p/'report.json').exists())=={'sim':1,'worker':1}
roles=collections.defaultdict(list)
for h in heaps:roles[h['role']].append(peak(h))
out={'source_sha256':source,'scope':'same-source full-controller smoke diagnostic audit, L1 absent','normal_heap_records':len(heaps),'normal_component_evidence':normal,'fault_instances_without_normal_heap_footer':missing,'role_whole_Rust_heap_peaks_plus_tail_including_model_work':{k:max(v) for k,v in roles.items()},'component_caps_addition_only':{'formal_Rust_joint_nominal_bytes':8*MiB,'joint_with_1MiB_Python_instead_of_0_25MiB_Rust_supervisor_nominal_bytes':8*MiB+3*MiB//4,'not_measured_joint_proof':True},'shared_8MiB_joint':'not_accepted','normal_3_10GiB':'not_accepted; all ordinary Engine/serde/helper allocations remain extra','L1_joint':'absent','formal_latency':'not_accepted; shortened cadence; execution concurrency conditions recorded in STATUS.md','eight_hour':'not_run','real_16GB':'not_run','full_M0':'not_accepted'}
print(json.dumps(out,indent=2))
