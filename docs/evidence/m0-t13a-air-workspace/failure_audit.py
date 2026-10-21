#!/usr/bin/env python3
"""Preserve actual failed prefix, including failures before any business starts."""
import collections,hashlib,json,sys,zipfile
from pathlib import Path
D=Path(__file__).resolve().parent;R=Path(sys.argv[1]);M=R/'model';B=M/'business';load=lambda p:json.loads(p.read_text());sha=lambda b:hashlib.sha256(b).hexdigest();info=load(D/'source.json');o=load(R/'observer-report.json')
assert o['status']=='failed' and o['model_exit_code']!=0 and o['binary_sha256']==info['binary_sha256'] and o['binary_unchanged'] and o['observer_source_unchanged']
heaps=[json.loads(l) for l in (M/'heaps.jsonl').open()] if (M/'heaps.jsonl').exists() else []
assert all(h['source_sha256']==info['source_sha256'] for h in heaps)
rows=[json.loads(l) for l in (B/'transcript.jsonl').open()] if (B/'transcript.jsonl').exists() else []
verified=[r['data'] for r in rows if 'endurance_verified_target' in r['data']]
if (B/'cpu-targets.jsonl').exists():
 reference=[json.loads(l) for l in (B/'cpu-targets.jsonl').open()]
 assert len(verified)<=723 and [v['expected'] for v in verified]==reference[:len(verified)]
 for v in verified:assert v['state']['business_sha256']==v['expected']['business_sha256'] and v['state']['background']['state_sha256']==v['expected']['air_sha256']
 with zipfile.ZipFile(D.parent/'m0-t13a-broker/t13a3m-functional-01/artifacts.zip') as z:
  assert (B/'cpu-final-engine.json').read_bytes()==z.read('model/business/cpu-final-engine.json') and (B/'cpu-targets.jsonl').read_bytes()==z.read('model/business/cpu-targets.jsonl')
events=[json.loads(l) for l in (M/'layout/events.jsonl').open()] if (M/'layout/events.jsonl').exists() else []
scans=[r for r in events if 'scan' in r];durable=[r for r in events if 'snapshot_durable' in r];admitted=[r for r in events if 'snapshot_admitted' in r]
for n,r in enumerate(scans):assert r['scan_deadline_seconds']==n and r['scan']['native_records']==361272 and r['scan']['stub_records']==1232426 and r['scan']['iteration']==n+1
for r in durable:assert r['snapshot_durable']['complete_record_restore']=='passed' and r['snapshot_durable']['full_pressure_file_readback']=='passed'
pool=[json.loads(l) for l in (M/'normal-pool/events.jsonl').open()]
for r in pool:
 assert r['summary']['available_touched_bytes']+r['summary']['granted_capacity_bytes']==64*1024**2
 field='pool_touched_bytes' if r.get('schema')=='mycity-normal-workspace-v1' else 'owner_actual_capacity_bytes'
 # An injected partial-return failure may be intentionally unbalanced, not used in this normal probe.
 assert r[field]==r['summary']['available_touched_bytes']
missing=[];normal_ids={(h['pid'],h['instance_unix_ns']) for h in heaps}
if (B/'diagnostics').exists():
 for p in (B/'diagnostics').iterdir():
  if not p.is_dir():continue
  with (p/'transactions.jsonl').open() as f:r=json.loads(f.readline())
  assert r['phase']=='producer_identity';i=r['identity'];assert i['source_sha256']==info['source_sha256']
  if (i['process_pid'],i['process_instance_unix_ns']) not in normal_ids:missing.append({'producer':p.name,'heap':'missing; not zero','sealed_report':(p/'report.json').exists()})
print(json.dumps({'source_sha256':info['source_sha256'],'result':'failed; not completed joint validation','reason':o['reason'],'actual_wall_seconds':o['actual_wall_seconds'],'pressure_seconds':o['maximum_sampled_non_normal_pressure_seconds'],'swap_net_growth_bytes':o['maximum_observed_swap_net_growth_bytes'],'verified_target_prefix':len(verified),'last_verified_game_second':verified[-1]['endurance_verified_target'] if verified else None,'completed_prefix_matches_original_cpu':True,'normal_heap_records':len(heaps),'missing_heap_instances':missing,'completed_L1_scans':len(scans),'completed_L1_snapshots':len(durable),'admitted_L1_snapshots':len(admitted),'pool_last_observed':pool[-1]['summary'],'pool_final_balance':'not_proven; forced or failed terminal is not success','latest_layout_cold_restore':'not_proven','joint':'failed','eight_hour':'not_run','real_16GB':'not_run','full_M0':'not_accepted'},indent=2))
