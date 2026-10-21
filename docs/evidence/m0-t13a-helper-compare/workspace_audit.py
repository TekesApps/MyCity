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
assert len(by)==6
helpers={g:es for g,es in by.items() if es[0]['owner_kind']=='helper_compare'}
assert len(helpers)==2
by={g:es for g,es in by.items() if es[0]['owner_kind']=='sim_codec'}
assert len(by)==4
heap_rows=[json.loads(l) for l in (M/'heaps.jsonl').open()]
helper_facts=[]
for g,es in helpers.items():
 assert [e['kind'] for e in es]==['prepared_before_helper_spawn','helper_actual_compare_slots_filled','retiring_after_helper_wait_or_cancel','returned_after_helper_wait_or_cancel']
 assert [e['phase'] for e in es]==['prepared','filled','retiring','returned']
 assert all(e['capacity_bytes']==4*MiB and e['owner_kind']=='helper_compare' for e in es)
 assert all(a['observed_ns']<=b['observed_ns'] for a,b in zip(es,es[1:]))
 i=es[1]['identity'];assert i==es[2]['identity']==es[3]['identity'] and i['run_epoch']==g
 assert es[0]['pool_touched_bytes']==es[1]['pool_touched_bytes']==60*MiB and es[3]['pool_touched_bytes']==64*MiB
 matches=[]
 for mode in ['before-reopen','compare-endurance']:
  x=load(M/'business'/mode/'result.json')
  if (x['pid'],x['instance_unix_ns'])==(i['process_pid'],i['process_instance_unix_ns']):matches.append(mode)
 assert len(matches)==1
 mode=matches[0];x=load(M/'business'/mode/'workspace.json')
 assert x['identity']==i and x['owner_capacity_bytes']==x['available_touched_bytes']==4*MiB
 assert x['full_balance_scan_passed'] and x['lent_capacity_bytes']==0
 assert all(s['claims']==s['returns']==1 for s in x['slots'])
 assert all(s['capacity_bytes']==s['available_touched_bytes']==2*MiB for s in x['slots'])
 assert [s['maximum_payload_bytes'] for s in x['slots']]==([2*MiB,0] if mode=='before-reopen' else [2*MiB,2*MiB])
 h=next(h for h in heap_rows if (h['pid'],h['instance_unix_ns'])==(i['process_pid'],i['process_instance_unix_ns']))
 peak=h['heap']['lifetime_peak_requested_bytes']+h['tail_reserved_bytes']
 assert h['success'] and peak<=4*MiB+MiB//4
 helper_facts.append({'mode':mode,'generation':g,'whole_heap_plus_tail':peak,'owner_bytes':4*MiB})
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
 assert f['typed_instance']>0 and f['typed_capacity']==MiB
 assert f['summary']['available_touched_bytes']==55*MiB and f['summary']['granted_capacity_bytes']==9*MiB and f['summary']['workspace_capacity_bytes']==4*MiB
 assert (f['parent'],f['parent_instance']) in identities
 assert f['broker_generation']==identities[(f['parent'],f['parent_instance'])]['run_epoch']
# Every surviving worker must report loaded and reset state from the same
# actual kernel, tied to its real child's Filled receipt. Killed attempts stay missing.
typed=[]
for f in fills:
 i=f['worker_identity'];logs=[]
 for p in (M/'business/diagnostics').glob('worker-*'):
  parts=p.name.split('-')
  if str(i['process_pid']) not in parts: continue
  rows_here=[json.loads(l) for l in (p/'transactions.jsonl').open()]
  ident=next(x['identity'] for x in rows_here if x.get('phase')=='producer_identity')
  if (ident['process_pid'],ident['process_instance_unix_ns']) == (i['process_pid'],i['process_instance_unix_ns']):
   logs += [x for x in rows_here if x.get('phase')=='worker_air_workspace']
 for x in logs:
  loaded,reset=x['loaded'],x['reset']
  assert loaded['workspace_instance']==reset['workspace_instance']==f['typed_instance']
  for y in (loaded,reset):
   assert y['payload_capacity_bytes']==MiB and y['typed_backing_bytes']+y['padding_bytes']==MiB
   assert y['full_fill_scan_passed'] and y['allocation_set_stable']
  assert loaded['loaded'] and loaded['active_cells']==128**2
  assert loaded['completed_restores']==reset['completed_restores']==1 and loaded['failed_restores']==reset['failed_restores']==0
  assert loaded['pages']==reset['pages'] and loaded['pages']>0
  assert not reset['loaded'] and reset['active_sources']==reset['active_cells']==0
  assert reset['inactive_typed_bytes']==20000*20+128**2*16
 typed.append({'pid':i['process_pid'],'instance':i['process_instance_unix_ns'],'workspace_instance':f['typed_instance'],'completed_kernel_logs':len(logs)})
assert sum(x['completed_kernel_logs'] for x in typed)==724 # 725 attempts; Cells fault cannot finish, committed-reply fault follows completed computation
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
assert s['workspace_grants']==s['workspace_fills']==s['workspace_returns']==6
assert s['grants']==s['fill_acknowledgements']==s['returns']==725 and s['available_touched_bytes']==64*MiB and s['workspace_capacity_bytes']==s['granted_capacity_bytes']==0
print(json.dumps({'fixed_helpers':helper_facts,'actual_sim_epochs':epochs,'actual_sim_traces':sim,'logical_windows':724,'worker_grants_including_fault_attempt':725,'fault_trace_completeness':'killed process may lack final slot trace or footer; not inferred zero or complete','dual_owner_pool_bytes':55*MiB,'final_pool_bytes':64*MiB,'typed_workers':typed,'normal64_common8':'not_accepted; Engine/serde/parser/Reference/FaultReference EXTRA; only fixed compare helper backs are in pool'},indent=2))
