#!/usr/bin/env python3
"""Audit preserved pressure failure without manufacturing a completed trajectory."""
import collections,hashlib,json,sys,zipfile
from pathlib import Path
D=Path(__file__).resolve().parent;R=Path(sys.argv[1]);M=R/'model';B=M/'business';load=lambda p:json.loads(p.read_text());sha=lambda b:hashlib.sha256(b).hexdigest();info=load(D/'source.json');o=load(R/'observer-report.json')
assert o['status']=='failed' and o['reason']=='system non-normal pressure exceeded 60 seconds' and o['model_exit_code']==-15
assert o['binary_sha256']==info['binary_sha256'] and o['binary_unchanged'] and o['observer_source_unchanged']
assert o['maximum_sampled_non_normal_pressure_seconds']>60
assert not (M/'report.json').exists() and not (B/'report.json').exists()
heaps=[json.loads(l) for l in (M/'heaps.jsonl').open()];assert all(h['source_sha256']==info['source_sha256'] and h['success'] for h in heaps)
normal_ids={(h['pid'],h['instance_unix_ns']) for h in heaps};assert len(heaps)==len(normal_ids)
producer_keys=set();sealed=0;missing=[]
for p in (B/'diagnostics').iterdir():
 if not p.is_dir():continue
 with (p/'transactions.jsonl').open('rb') as f:line=f.readline(1025)
 assert len(line)<=1024 and line.endswith(b'\n');row=json.loads(line);assert row['phase']=='producer_identity';x=row['identity']
 assert x['source_sha256']==info['source_sha256'] and p.name==f"{x['producer']}-{x['process_pid']}-{x['producer_instance']}"
 k=(x['process_pid'],x['process_instance_unix_ns']);assert k not in producer_keys;producer_keys.add(k)
 if (p/'report.json').exists():
  report=load(p/'report.json');assert all(report[n]==x[n] for n in ['source_sha256','producer','process_pid','process_instance_unix_ns','producer_instance']);sealed+=1
  assert report['transaction_log_bytes']==(p/'transactions.jsonl').stat().st_size
 if k not in normal_ids:missing.append({'directory':p.name,'pid':k[0],'instance':k[1],'report_present':(p/'report.json').exists(),'heap':'missing, not zero'})
rows=[json.loads(l) for l in (B/'transcript.jsonl').open()];verified=[r['data'] for r in rows if 'endurance_verified_target' in r['data']]
reference=[json.loads(l) for l in (B/'cpu-targets.jsonl').open()]
assert verified and len(verified)<723
assert [v['expected'] for v in verified]==reference[:len(verified)]
for v in verified:
 assert v['state']['business_sha256']==v['expected']['business_sha256'] and v['state']['background']['state_sha256']==v['expected']['air_sha256']
with zipfile.ZipFile(D.parent/'m0-t13a-broker/t13a3m-functional-01/artifacts.zip') as z:
 assert (B/'cpu-final-engine.json').read_bytes()==z.read('model/business/cpu-final-engine.json')
 assert (B/'cpu-targets.jsonl').read_bytes()==z.read('model/business/cpu-targets.jsonl')
events=[json.loads(l) for l in (M/'layout/events.jsonl').open()];scans=[r for r in events if 'scan' in r];durable=[r for r in events if 'snapshot_durable' in r];admitted=[r for r in events if 'snapshot_admitted' in r]
for n,r in enumerate(scans):
 assert r['scan_deadline_seconds']==n and r['scan']['native_records']==361272 and r['scan']['stub_records']==1232426 and r['scan']['iteration']==n+1
for r in durable:assert r['snapshot_durable']['complete_record_restore']=='passed' and r['snapshot_durable']['full_pressure_file_readback']=='passed'
assert len(scans)<600 and admitted[0]['snapshot_deadline_seconds']==0
pool=[json.loads(l) for l in (M/'normal-pool/events.jsonl').open()]
for r in pool:
 assert r['summary']['available_touched_bytes']+r['summary']['granted_capacity_bytes']==64*1024**2
 assert r['owner_actual_capacity_bytes']==r['summary']['available_touched_bytes']
last=pool[-1]['summary'];common=[json.loads(l) for l in (M/'common-events.jsonl').open()]
assert not any(r['kind']=='layout_stop_sent' for r in common)
print(json.dumps({'source_sha256':info['source_sha256'],'result':'failed_pressure_protection; not completed joint validation','actual_wall_seconds':o['actual_wall_seconds'],'pressure_seconds':o['maximum_sampled_non_normal_pressure_seconds'],'swap_net_growth_bytes':o['maximum_observed_swap_net_growth_bytes'],'verified_target_prefix':len(verified),'last_verified_game_second':verified[-1]['endurance_verified_target'],'all_completed_prefix_business_air_equal_original_cpu':True,'normal_heap_records':len(heaps),'initialized_producers':len(producer_keys),'sealed_reports':sealed,'missing_footer_producers':missing,'completed_L1_scans':len(scans),'completed_L1_snapshots':len(durable),'admitted_L1_snapshots':len(admitted),'pool_last_observed':last,'pool_final_balance':'not_proven; forced terminal, no success report','latest_layout_cold_restore':'not_run','complete_30days':'not_completed','600_second_joint':'failed','eight_hour':'not_run','real_16GB':'not_run','full_M0':'not_accepted'},indent=2))
