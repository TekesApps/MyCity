#!/usr/bin/env python3
"""Actual full-controller owners, including killed attempts; not total heap acceptance."""
import collections,json,sys
from pathlib import Path
R=Path(sys.argv[1]);M=R/'model';MiB=1024**2
load=lambda p:json.loads(p.read_text())
rows=[json.loads(l) for l in (M/'normal-pool/events.jsonl').open()];by=collections.defaultdict(list)
for r in rows:
 assert r['summary']['available_touched_bytes']+r['summary']['granted_capacity_bytes']==64*MiB
 if r.get('schema')=='mycity-normal-workspace-v1':by[r['generation']].append(r)
assert len(by)==4
identities={};epochs=[]
for gen,es in by.items():
 assert [x['phase'] for x in es]==['prepared','filled','retiring','returned']
 assert [x['kind'] for x in es]==['prepared_before_sim_spawn','sim_actual_codec_slots_filled','retiring_after_sim_wait_or_cancel','returned_after_sim_wait_or_cancel']
 assert all(x['capacity_bytes']==4*MiB and x['owner_kind']=='sim_codec' for x in es)
 assert all(a['observed_ns']<=b['observed_ns'] for a,b in zip(es,es[1:]))
 p,f,t,r=es;i=f['identity'];assert i['run_epoch']==gen and i['process_pid']==f['pid'] and i['process_instance_unix_ns']>0
 assert t['identity']==r['identity']==i and f['pool_touched_bytes']==60*MiB and r['pool_touched_bytes']==64*MiB
 identities[(i['process_pid'],i['process_instance_unix_ns'])]=i
 epochs.append({'generation':gen,'prepared_to_filled_ns':f['transition_ns'],'observed_return_ns':r['observed_ns']})
fills=[r for r in rows if r['kind']=='worker_actual_slots_filled'];assert len(fills)==725
for f in fills:
 assert f['summary']['available_touched_bytes']==56*MiB and f['summary']['granted_capacity_bytes']==8*MiB and f['summary']['workspace_capacity_bytes']==4*MiB
 assert (f['parent'],f['parent_instance']) in identities
 assert f['broker_generation']==identities[(f['parent'],f['parent_instance'])]['run_epoch']
sim=[]
for p in (M/'business/diagnostics').glob('sim-*'):
 owners=[]
 for line in (p/'transactions.jsonl').open():
  r=json.loads(line)
  if r.get('phase')=='sim_controlled_slots':owners.append(r['owner'])
 assert owners
 i=owners[0]['identity'];key=(i['process_pid'],i['process_instance_unix_ns']);assert i==identities[key]
 complete=0
 for o in owners:
  assert o['owner_capacity_bytes']==o['available_touched_bytes']==4*MiB and o['lent_capacity_bytes']==0 and o['full_balance_scan_passed']
  assert (o['identity']['process_pid'],o['identity']['process_instance_unix_ns'])==key and o['identity']['run_epoch']==i['run_epoch']
  assert all(s['capacity_bytes']==s['available_touched_bytes']==2*MiB and s['claims']==s['returns'] for s in o['slots'])
  if all(s['claims']==1 for s in o['slots']):
   assert all(0<s['maximum_payload_bytes']<=2*MiB for s in o['slots']);complete+=1
 sim.append({'pid':key[0],'instance':key[1],'slot_reports':len(owners),'complete_slot_trace_jobs':complete,'sealed':(p/'report.json').exists()})
assert {(x['pid'],x['instance']) for x in sim}==set(identities)
s=load(M/'normal-pool/report.json')['summary']
assert s['workspace_grants']==s['workspace_fills']==s['workspace_returns']==4
assert s['grants']==s['fill_acknowledgements']==s['returns']==725 and s['available_touched_bytes']==64*MiB and s['workspace_capacity_bytes']==s['granted_capacity_bytes']==0
print(json.dumps({'actual_sim_epochs':epochs,'actual_sim_traces':sim,'logical_windows':724,'worker_grants_including_fault_attempt':725,'fault_trace_completeness':'killed process may lack final slot trace or footer; not inferred zero or complete','dual_owner_pool_bytes':56*MiB,'final_pool_bytes':64*MiB,'normal64_common8':'not_accepted; Engine/serde/helper EXTRA'},indent=2))
