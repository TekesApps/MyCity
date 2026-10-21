#!/usr/bin/env python3
"""Audit actual Sim and worker owners separately; not total-memory acceptance."""
import collections,json,sys
from pathlib import Path
R=Path(sys.argv[1]);M=R/'model';load=lambda p:json.loads(p.read_text());MiB=1024**2
rows=[json.loads(l) for l in (M/'normal-pool/events.jsonl').open()]
work=[r for r in rows if r.get('schema')=='mycity-normal-workspace-v1']
by=collections.defaultdict(list)
for r in work:by[r['generation']].append(r)
assert len(by)==2
identities={};timings=[]
for gen,events in by.items():
 assert [r['kind'] for r in events]==['prepared_before_sim_spawn','sim_actual_codec_slots_filled','retiring_after_sim_wait_or_cancel','returned_after_sim_wait_or_cancel']
 assert [r['phase'] for r in events]==['prepared','filled','retiring','returned']
 assert all(r['owner_kind']=='sim_codec' and r['capacity_bytes']==4*MiB for r in events)
 assert all(a['observed_ns']<=b['observed_ns'] for a,b in zip(events,events[1:]))
 prepare,filled,retiring,returned=events
 assert prepare['pid'] is None and prepare['identity'] is None
 identity=filled['identity'];assert identity['process_pid']==filled['pid'] and identity['run_epoch']==gen and identity['process_instance_unix_ns']>0
 assert retiring['identity']==returned['identity']==identity
 assert filled['pool_touched_bytes']==60*MiB and returned['pool_touched_bytes']==64*MiB
 assert returned['summary']['workspace_capacity_bytes']==0
 identities[(identity['process_pid'],identity['process_instance_unix_ns'])]=identity
 timings.append({'generation':gen,'prepare_to_filled_ns':filled['transition_ns'],'lifetime_to_return_ns':returned['transition_ns']})
worker_fills=[r for r in rows if r['kind']=='worker_actual_slots_filled']
assert len(worker_fills)==724,collections.Counter(r['kind'] for r in rows)
for r in worker_fills:
 assert r['summary']['available_touched_bytes']==56*MiB and r['summary']['granted_capacity_bytes']==8*MiB and r['summary']['workspace_capacity_bytes']==4*MiB
simlogs=[];jobs=0
for d in (M/'business/diagnostics').iterdir():
 if not d.is_dir():continue
 reports=[]
 for line in (d/'transactions.jsonl').open():
  x=json.loads(line)
  if x.get('phase')=='sim_controlled_slots':reports.append(x['owner'])
 if not reports:continue
 first=reports[0];i=first['identity'];key=(i['process_pid'],i['process_instance_unix_ns'])
 assert i==identities[key]
 for r in reports:
  assert r['owner_capacity_bytes']==r['available_touched_bytes']==4*MiB and r['lent_capacity_bytes']==0 and r['full_balance_scan_passed']
  assert all(s['capacity_bytes']==s['available_touched_bytes']==2*MiB and s['claims']==s['returns'] for s in r['slots'])
  assert (r['identity']['process_pid'],r['identity']['process_instance_unix_ns'])==key
 final=[r for r in reports if all(s['claims']==1 for s in r['slots'])]
 for r in final:assert all(0<s['maximum_payload_bytes']<=2*MiB for s in r['slots'])
 jobs+=len(final);simlogs.append({'pid':key[0],'instance':key[1],'reports':len(reports),'complete_jobs':len(final)})
assert len(simlogs)==2 and jobs==724,(simlogs,jobs)
assert {(s['pid'],s['instance']) for s in simlogs}==set(identities)
summary=load(M/'normal-pool/report.json')['summary']
assert summary['workspace_grants']==summary['workspace_fills']==summary['workspace_returns']==2
assert summary['grants']==summary['fill_acknowledgements']==summary['returns']==724
assert summary['available_touched_bytes']==64*MiB and summary['granted_capacity_bytes']==summary['workspace_capacity_bytes']==0
print(json.dumps({'actual_workspace_epochs':timings,'actual_sim_logs':simlogs,'actual_worker_fills':len(worker_fills),'actual_complete_sim_jobs':jobs,'dual_owner_pool_bytes':56*MiB,'final_pool_bytes':64*MiB,'Engine_serde_helper':'EXTRA','normal64_common8_fulljoint_8h_16GB':'not_accepted'},indent=2))
