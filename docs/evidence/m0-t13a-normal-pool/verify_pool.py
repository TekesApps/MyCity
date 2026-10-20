#!/usr/bin/env python3
"""Independent actual page/grant/worker-instance audit, not exact normal-owned approval."""
import collections,json,sys
from pathlib import Path
R=Path(sys.argv[1]);M=R/'model';P=M/'normal-pool';load=lambda p:json.loads(p.read_text());MiB=1024**2
ready=load(P/'ready.json');final=load(P/'report.json');business=load(M/'business/report.json');rows=[json.loads(s) for s in (P/'events.jsonl').read_text().splitlines()]
assert ready['schema']=='mycity-normal-pool-v1' and ready['source']==business['source_sha256']==final['source_sha256'] and ready['fault_injection'] is None
assert ready['capacity_bytes']==64*MiB and ready['diagnostic_reserved_bytes']==MiB//4
assert final['balanced_after_owned_wait'] and final['normal_owned_acceptance'].startswith('not_accepted')
assert final['summary']['grants']==final['summary']['returns']==final['summary']['fill_acknowledgements']==724
assert final['summary']['available_touched_bytes']==64*MiB and final['summary']['granted_capacity_bytes']==0 and final['summary']['rejected_requests']==0
assert rows[0]['kind']=='created' and rows[-1]['kind']=='parent_pipe_closed'
assert all(a['observed_ns']<=b['observed_ns'] for a,b in zip(rows,rows[1:]))
heaps={(h['pid'],h['instance_unix_ns']):h for h in (json.loads(s) for s in (M/'heaps.jsonl').read_text().splitlines())}
assert heaps[(ready['pid'],ready['boot'])]['role']=='normal-pool-host'
assert heaps[(ready['pid'],ready['boot'])]['success']
assert heaps[(ready['pid'],ready['boot'])]['heap']['lifetime_peak_requested_bytes']+65536<=64*MiB+MiB//4
owners={}
for p in (M/'business/diagnostics').glob('worker-*/transactions.jsonl'):
 for line in p.read_text().splitlines():
  v=json.loads(line)
  if v.get('phase')=='worker_controlled_slots':
   o=v['owner'];i=o['identity'];key=(i['process_pid'],i['process_instance_unix_ns']);assert key not in owners;owners[key]=o
assert len(owners)==724
active=None;grants={};gaps=[];scans=0
for row in rows:
 s=row['summary'];kind=row['kind'];assert s['available_touched_bytes']+s['granted_capacity_bytes']==64*MiB
 # A return event is only after replacement pages have actually been touched.
 assert row['owner_actual_capacity_bytes']==s['available_touched_bytes']
 if kind=='grant_released_before_spawn':
  assert active is None and row['id']==len(grants)+1 and s['available_touched_bytes']==60*MiB and s['granted_capacity_bytes']==4*MiB
  active={'release':row,'attached':None,'filled':None,'wait':None};grants[row['id']]=active
 elif kind=='child_attached':
  assert active and active['attached'] is None and row['id']==active['release']['id'] and row['pid']>0;active['attached']=row
 elif kind=='worker_actual_slots_filled':
  assert active and active['attached'] and active['filled'] is None and row['id']==active['release']['id']
  i=row['worker_identity'];key=(row['pid'],i['process_instance_unix_ns']);assert key in owners and owners[key]['identity']==i and owners[key]['owner_capacity_bytes']==4*MiB and owners[key]['available_touched_bytes']==4*MiB and owners[key]['full_balance_scan_passed']
  assert row['pid']==active['attached']['pid']==i['process_pid']
  parent=active['release'];assert heaps[(parent['parent'],parent['parent_instance'])]['role']=='sim-host-scale-full'
  assert heaps[key]['role']=='compute-worker-bounded' and heaps[key]['success'];active['filled']=row;gaps.append(row['observed_ns']-parent['observed_ns'])
 elif kind=='owned_child_wait_success':
  assert active and active['filled'] and active['wait'] is None and row['pid']==active['attached']['pid'];active['wait']=row
 elif kind=='returned_after_wait_or_abort':
  assert active and active['wait'] and row['id']==active['release']['id'] and row['pid']==active['attached']['pid'] and s['available_touched_bytes']==64*MiB and s['granted_capacity_bytes']==0;active=None
 elif kind=='periodic_balance_scan':scans+=1
 else:assert kind in ['created','parent_pipe_closed'],kind
assert active is None and len(grants)==724 and scans>0
print(json.dumps({'source_sha256':business['source_sha256'],'status':'actual_64MiB_balance_and_724_worker_grants_verified','pool_process_identity':[ready['pid'],ready['boot']],'capacity_bytes':64*MiB,'worker_grant_bytes':4*MiB,'grants_with_actual_slot_identity_and_owned_wait':len(grants),'periodic_full_page_scans':scans,'maximum_observed_release_event_to_fill_event_ns':max(gaps),'pool_reported_prepare_to_fill_maximum_ns':final['summary']['maximum_prepare_to_fill_or_reap_ns'],'transition_scope':'observed event intervals; no claim of continuously full pressure or exact allocation timestamps','pool_requested_heap_peak_plus_tail_bytes':heaps[(ready['pid'],ready['boot'])]['heap']['lifetime_peak_requested_bytes']+65536,'unconverted_Engine_work':'extra_pressure_not_deducted','exact_normal_owned':'not_accepted','formal_steady':'not_run','four_speeds':'not_run','eight_hour':'not_run','real_16GB':'not_run'},indent=2))
