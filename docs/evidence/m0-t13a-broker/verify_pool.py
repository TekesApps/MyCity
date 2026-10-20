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
active=None;grants={};gaps=[];scans=0;session=None;pending_ack=None;sessions={};last_generation=0
for row in rows:
 s=row['summary'];kind=row['kind'];assert s['available_touched_bytes']+s['granted_capacity_bytes']==64*MiB
 # A return event is only after replacement pages have actually been touched.
 assert row['owner_actual_capacity_bytes']==s['available_touched_bytes']
 if kind=='broker_session_registered':
  assert session is None and pending_ack is None and active is None
  generation=row['broker_generation'];assert generation>last_generation;last_generation=generation
  session={'generation':generation,'pid':None,'grants':0};sessions[generation]=session
 elif kind=='broker_sim_bound':
  assert session and row['broker_generation']==session['generation'] and row['broker_sim_pid']>0
  assert session['pid'] in (None,row['broker_sim_pid']);session['pid']=row['broker_sim_pid']
 elif kind=='broker_session_revoked_after_sim_wait':
  assert session and active is None and pending_ack is None and row['broker_generation']==session['generation'] and row['broker_sim_pid']==session['pid']
  pending_ack=session['generation'];session=None
 elif kind=='broker_revoke_receipt_acknowledged':
  assert session is None and pending_ack==row['broker_generation'];pending_ack=None
 elif kind=='grant_released_before_spawn':
  assert active is None and row['id']==len(grants)+1 and s['available_touched_bytes']==60*MiB and s['granted_capacity_bytes']==4*MiB
  assert session and session['pid']==row['parent']==row['broker_sim_pid'] and session['generation']==row['broker_generation']
  session['grants']+=1
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
assert active is None and session is None and pending_ack is None and len(grants)==724 and scans>0
assert len(sessions)==2 and sum(s["grants"] for s in sessions.values())==724
assert all(len(line)+1<=1536 for line in (P/"events.jsonl").read_bytes().splitlines())
assert (P/"events.jsonl").stat().st_size<=8*MiB
print(json.dumps({'source_sha256':business['source_sha256'],'status':'actual_64MiB_balance_and_724_worker_grants_verified','pool_process_identity':[ready['pid'],ready['boot']],'capacity_bytes':64*MiB,'worker_grant_bytes':4*MiB,'grants_with_actual_slot_identity_and_owned_wait':len(grants),'periodic_logged_scan_checkpoints':scans,'actual_total_balance_scans':final['summary']['scans'],'broker_sessions':list(sessions.values()),'event_log_bytes':(P/'events.jsonl').stat().st_size,'event_row_bound_bytes':1536,'scan_cadence_seconds':1,'periodic_log_cadence_seconds':30,'maximum_observed_release_event_to_fill_event_ns':max(gaps),'pool_reported_prepare_to_fill_maximum_ns':final['summary']['maximum_prepare_to_fill_or_reap_ns'],'transition_scope':'observed event intervals; no claim of continuously full pressure or exact allocation timestamps','pool_requested_heap_peak_plus_tail_bytes':heaps[(ready['pid'],ready['boot'])]['heap']['lifetime_peak_requested_bytes']+65536,'unconverted_Engine_work':'extra_pressure_not_deducted','exact_normal_owned':'not_accepted','formal_steady':'not_run','four_speeds':'not_run','eight_hour':'not_run','real_16GB':'not_run'},indent=2))
