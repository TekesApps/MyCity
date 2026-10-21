#!/usr/bin/env python3
"""Measure this controller's component evidence without accepting absent/joint budgets."""
import collections,json,sys
from pathlib import Path
R=Path(sys.argv[1]);M=R/'model';B=M/'business';load=lambda p:json.loads(p.read_text());MiB=1024**2
heaps=[json.loads(s) for s in (M/'heaps.jsonl').read_text().splitlines()]
INFO=Path(sys.argv[2]) if len(sys.argv)>2 else Path(__file__).resolve().parent;info=load(INFO/'source.json');source=info['source_sha256'];assert all(h['source_sha256']==source for h in heaps)
manifest=load(M/'manifest.json');layout_manifest=load(M/'layout/manifest.json')
assert manifest['external_owned_charge_bytes']==int(3.25*MiB)
assert manifest['normal_pool_bytes']==64*MiB
assert layout_manifest['external_owned_charge_bytes']==64*MiB and layout_manifest['metadata_and_io_charge_bytes']==4*MiB
old=load(Path('/Users/frankqin/dev/MyCity/work/t13a70-joint-01/model/layout/snapshots/snapshot-000002/index.json'))['manifest']
def layout_formula(m):
 return m['native']['owned_capacity_bytes']+sum(x['owned_capacity_bytes'] for x in m['stubs'])+m['cow_reserve_bytes']+m['metadata_and_io_charge_bytes']+m['external_owned_charge_bytes']+m['padding_payload_total_bytes']
assert layout_formula(layout_manifest)==layout_manifest['normal_joint_budget_bytes']==old['normal_joint_budget_bytes']
for k in ['native','stubs','population','cow_reserve_bytes','metadata_and_io_charge_bytes','external_owned_charge_bytes','padding_payload_bytes','padding_payload_total_bytes']:
 assert layout_manifest[k]==old[k],k
snapshot_facts=[]
for folder in sorted((M/'layout/snapshots').glob('snapshot-*')):
 assert not folder.name.endswith('.pending')
 idx=load(folder/'index.json');m=idx['manifest'];assert layout_formula(m)==m['normal_joint_budget_bytes']
 assert m['padding_payload_total_bytes']==old['padding_payload_total_bytes']
 pressure=next(x for x in idx['files'] if x['name']=='pressure.bin')
 assert pressure['bytes']==(folder/'pressure.bin').stat().st_size==m['padding_payload_total_bytes']
 assert m['external_owned_charge_bytes']==64*MiB
 snapshot_facts.append(dict(path=str(folder),pressure_bytes=pressure['bytes'],external_charge_bytes=m['external_owned_charge_bytes']))
assert len(snapshot_facts)==2
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
 helper.append({'mode':mode,'control_capacity':c,'whole_Rust_heap_peak_plus_tail_bytes_including_model_work':peak(h),'scope':('fixed6MiB whole Rust arena including diagnostic allocations, independently audited' if mode in ['before-reopen','compare-endurance'] else 'fixed20MiB whole Rust arena including diagnostic allocations, independently audited')})
normal['helper']=helper
pool=next(h for h in heaps if h['role']=='normal-pool-host');assert peak(pool)<=64*MiB+MiB//4
normal['pool']={'whole_Rust_heap_peak_plus_tail_bytes':peak(pool),'whole_upper_bound_bytes':64*MiB+MiB//4,'diagnostic_0_25MiB_separate_proof':'not_closed; no subtraction of lifetime peaks while pages are lent'}
observer=load(R/'observer-report.json');joint=load(M/'report.json')
assert observer['observer_hot_bound_bytes']<=2*MiB
normal['observer']={'observed_hot_trace_bound_bytes':observer['observer_hot_bound_bytes'],'cap_bytes':2*MiB,'startup_native_foreign':'not_proven_by_hot_tracemalloc'}
supervisor=next(h for h in heaps if h['role']=='joint-endurance');assert peak(supervisor)<=MiB//4
normal['Rust_supervisor']={'whole_Rust_heap_peak_plus_tail_bytes':peak(supervisor),'cap_bytes':MiB//4,'native_stack_foreign':'separate'}
layout=load(M/'layout/report.json');assert layout['diagnostics']['charged_bound_bytes']<=3*MiB//4
normal['layout']=layout['diagnostics']
missing=[];normal_ids={(h['pid'],h['instance_unix_ns']) for h in heaps}
for p in (B/'diagnostics').iterdir():
 if not p.is_dir():continue
 identity=json.loads((p/'transactions.jsonl').open().readline())['identity'];k=(identity['process_pid'],identity['process_instance_unix_ns'])
 if k not in normal_ids:missing.append({'producer':p.name,'pid':k[0],'process_instance_unix_ns':k[1],'sealed_component_report_present':(p/'report.json').exists(),'normal_heap_footer':'absent; not zero'})
waits=[json.loads(s) for s in (M/'normal-pool/events.jsonl').read_text().splitlines()]
failed=[r for r in waits if r['kind']=='owned_child_wait_failure']
missing_ids={(x['pid'],x['process_instance_unix_ns']) for x in missing}
assert (failed[0]['parent'],failed[0]['parent_instance']) in missing_ids
assert missing_ids<={(r['worker_identity']['process_pid'],r['worker_identity']['process_instance_unix_ns']) for r in failed}|{(failed[0]['parent'],failed[0]['parent_instance'])}
assert collections.Counter(p.name.split('-')[0] for p in (B/'diagnostics').iterdir() if p.is_dir() and not (p/'report.json').exists())=={'sim':1,'worker':1}
roles=collections.defaultdict(list)
for h in heaps:roles[h['role']].append(peak(h))
out={'actual_layout_and_diagnostic_ledgers':{'layout_external_charge_bytes':64*MiB,'layout_metadata_bytes':4*MiB,'supervisor_external_diagnostics_bytes':manifest['external_owned_charge_bytes'],'pressure_unchanged_from_v70':True,'actual_full_snapshots':snapshot_facts,'L1_metrics':'independent diagnostics, not charged again to the normal pool'},'source_sha256':source,'scope':'same-source full-controller joint smoke diagnostic audit, full L1 pressure present','normal_heap_records':len(heaps),'normal_component_evidence':normal,'fault_instances_without_normal_heap_footer':missing,'role_whole_Rust_heap_peaks_plus_tail_including_model_work':{k:max(v) for k,v in roles.items()},'external_without_observer_bytes':int(3.25*MiB),'sim_and_helper_diagnostics':'inside_role_arena_not_external_double_subtracted','component_caps_addition_only':{'formal_Rust_joint_nominal_bytes':8*MiB,'not_measured_joint_proof':True},'shared_8MiB_joint':'not_accepted','normal_3_10GiB':'not_accepted; fixed whole Rust arenas verified separately, native and stack outside arena','L1_joint':'present; pressure substitute is not product functionality','formal_latency':'not_accepted; shortened cadence; execution concurrency conditions recorded in STATUS.md','eight_hour':'not_run','real_16GB':'not_run','full_M0':'not_accepted'}
print(json.dumps(out,indent=2))
