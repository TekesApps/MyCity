#!/usr/bin/env python3
"""Actual role arenas, exact pool balance and completed worker backing traces."""
import collections,json,sys
from pathlib import Path
R=Path(sys.argv[1]);M=R/'model';MiB=1024**2
load=lambda p:json.loads(p.read_text())
rows=[json.loads(l) for l in (M/'normal-pool/events.jsonl').open()];by=collections.defaultdict(list)
for r in rows:
 assert r['summary']['available_touched_bytes']+r['summary']['granted_capacity_bytes']==64*MiB
 if r.get('schema')=='mycity-normal-role-arena-v2':by[r['generation']].append(r)
assert len(by)==8
assert collections.Counter(es[0]['owner_kind'] for es in by.values())=={'sim_codec':4,'model':2,'helper_compare':2}
heap_rows=[json.loads(l) for l in (M/'heaps.jsonl').open()]
failed=[r for r in rows if r['kind']=='owned_child_wait_failure']
assert failed
killed_sim=(failed[0]['parent'],failed[0]['parent_instance'])
identities={};facts=[]

for gen,es in by.items():
 assert [x['phase'] for x in es]==['prepared','filled','retiring','returned']
 p,f,t,r=es;i=f['identity'];kind=f['owner_kind'];cap={'sim_codec':59,'model':20,'helper_compare':6}[kind]*MiB
 assert all(x['capacity_bytes']==cap for x in es)
 assert all(a['observed_ns']<=b['observed_ns'] for a,b in zip(es,es[1:]))
 assert i['run_epoch']==gen and i['process_pid']==f['pid'] and i['process_instance_unix_ns']>0
 assert i==t['identity']==r['identity']
 assert f['pool_touched_bytes']==64*MiB-cap and r['pool_touched_bytes']==64*MiB
 a=f['arena'];act=a['activation']
 assert act['pid']==i['process_pid'] and act['arena_start']==i['owner_instance']
 assert act['arena_bytes']+act['bootstrap_charged_bytes']+act['adapter_header_bytes']==act['grant_bytes']==cap
 assert act['initialized_bytes']==act['arena_bytes']
 assert a['bootstrap_peak_bytes']+act['adapter_header_bytes']<=MiB
 assert a['bootstrap_live_bytes']+a['bootstrap_quarantined_bytes']==act['bootstrap_charged_bytes']
 assert a['arena_used_bytes']+a['arena_free_bytes']==a['arena_usable_bytes']<=act['arena_bytes']
 assert a['current_requested_bytes']<=a['lifetime_peak_requested_bytes']<=cap and a['allocation_failures']==0
 assert len(f['backing_sha256'])==64 and f['backing_sha256']==t['backing_sha256']==r['backing_sha256']
 hs=[h for h in heap_rows if (h['pid'],h['instance_unix_ns'])==(i['process_pid'],i['process_instance_unix_ns'])]
 assert len(hs)==(0 if (i['process_pid'],i['process_instance_unix_ns'])==killed_sim else 1)
 if hs:
  h=hs[0];assert h['role']==('sim-host-scale-full-probe' if kind=='sim_codec' else 'scale-model-task');end=h['role_arena'];assert end['activation']==act and end['allocation_failures']==0
  assert end['lifetime_peak_requested_bytes']<=cap and h['success']
 if kind=='sim_codec':identities[(i['process_pid'],i['process_instance_unix_ns'])]=i
 facts.append(dict(generation=gen,kind=kind,pid=i['process_pid'],grant_bytes=cap,initialized_arena_bytes=act['arena_bytes'],bootstrap_bytes=act['bootstrap_charged_bytes'],static_header_bytes=act['adapter_header_bytes'],surviving_footer=bool(hs)))
# All arena lifecycle facts tied to actual registration, bind, revoke and ACK.
positions={id(row):n for n,row in enumerate(rows)}
last_ack=-1
for gen,es in sorted(by.items()):
 p,f,t,r=es;ip,iff,it,ir=[positions[id(x)] for x in es]
 registered=[(n,x) for n,x in enumerate(rows) if x['kind']=='broker_session_registered' and x.get('broker_generation')==gen]
 bound=[(n,x) for n,x in enumerate(rows) if x['kind'] in ['broker_sim_bound','broker_helper_bound'] and x.get('broker_generation')==gen]
 revoked=[(n,x) for n,x in enumerate(rows) if x['kind']=='broker_session_revoked_after_sim_wait' and x.get('broker_generation')==gen]
 acks=[(n,x) for n,x in enumerate(rows) if x['kind']=='broker_revoke_receipt_acknowledged' and x.get('broker_generation')==gen]
 assert len(registered)==len(bound)==len(revoked)==len(acks)==1
 reg,bnd,rev,ack=registered[0],bound[0],revoked[0],acks[0]
 assert last_ack<ip<reg[0]<bnd[0]<iff<it<ir<rev[0]<ack[0]
 assert bnd[1]['broker_sim_pid']==f['identity']['process_pid']==rev[1]['broker_sim_pid']
 assert bnd[1]['kind']==('broker_sim_bound' if f['owner_kind']=='sim_codec' else 'broker_helper_bound')
 last_ack=ack[0]
fills=[r for r in rows if r['kind']=='worker_actual_slots_filled'];assert len(fills)==725
for f in fills:
 assert f['typed_instance']>0 and f['typed_capacity']==MiB
 assert f['summary']['available_touched_bytes']==0 and f['summary']['granted_capacity_bytes']==64*MiB and f['summary']['workspace_capacity_bytes']==59*MiB
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
 cells_fault=next(x for x in failed if x['worker_identity']['job']==load(M/'business/report.json')['clock']['retry_job'])['worker_identity']
 expected_logs=0 if (i['process_pid'],i['process_instance_unix_ns'])==(cells_fault['process_pid'],cells_fault['process_instance_unix_ns']) else 1
 assert len(logs)==expected_logs
 typed.append({'pid':i['process_pid'],'instance':i['process_instance_unix_ns'],'workspace_instance':f['typed_instance'],'completed_kernel_logs':len(logs)})
assert sum(x['completed_kernel_logs'] for x in typed)==724 # 725 attempts; Cells fault cannot finish, committed-reply fault follows completed computation
sim=[]
for p in (M/'business/diagnostics').glob('sim-*'):
 owners=[]
 for line in (p/'transactions.jsonl').open():
  r=json.loads(line)
  if r.get('phase')=='sim_controlled_slots':owners.append(r['owner'])
 assert owners
 i=owners[0]['identity'];key=(i['process_pid'],i['process_instance_unix_ns']);assert (i['process_pid'],i['process_instance_unix_ns'],i['run_epoch'])==(identities[key]['process_pid'],identities[key]['process_instance_unix_ns'],identities[key]['run_epoch'])
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
assert s['workspace_grants']==s['workspace_fills']==s['workspace_returns']==8
assert s['grants']==s['fill_acknowledgements']==s['returns']==725 and s['available_touched_bytes']==64*MiB and s['workspace_capacity_bytes']==s['granted_capacity_bytes']==0
print(json.dumps(dict(role_arenas=facts,worker_typed_backings=typed,sim_codec_traces=sim,workspace_lifecycles=8,worker_lifecycles=725,return_evidence='Frozen production Child wait/try_wait call chain plus actual process fault tests and identity-linked protocol trace; no independent kernel-wait timestamp',independent_wait_timestamp_observed=False,scope='Actual fixed Rust arenas plus startup charged prefix; native and stack remain separate, smoke not formal acceptance'),indent=2))
