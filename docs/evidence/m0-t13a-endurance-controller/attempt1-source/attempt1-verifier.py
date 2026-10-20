#!/usr/bin/env python3
"""Independent full-workload controller audit. Smoke never counts as endurance/joint/performance."""
import collections,hashlib,json,sys,zipfile
from pathlib import Path
D=Path(__file__).resolve().parent;R=Path(sys.argv[1]);M=R/'model';B=M/'business';load=lambda p:json.loads(p.read_text());sha=lambda b:hashlib.sha256(b).hexdigest()
info=load(D/'source.json');o=load(R/'observer-report.json');run=load(M/'runner-report.json');report=load(B/'report.json');clock=report['clock'];formal=clock['formal']
assert o['status']=='passed_measurement_wrapper' and o['binary_unchanged'] and o['observer_source_unchanged']
assert run['status']=='completed_functional_only' and run['rust_binary_sha256']==info['binary_sha256'] and run['source_sha256']==report['source_sha256']==info['source_sha256']
assert sha((D/'runner.py').read_bytes())==run['runner_sha256']==info['runner_sha256']
assert clock['hours']==720 and clock['targets']==723 and clock['queries_completed']==2880 and clock['saves_completed']==96 and clock['scheduled_cold_reopen']
assert clock['actual_wall_seconds']>=clock['required_wall_seconds']==(28800 if formal else 72)
assert report['entire_engine_including_metadata']=='equal_independent_fault_reference'
expected=load(B/'fault-reference/cpu-final-engine.json');baseline=load(B/'cpu-final-engine.json')
# No blanket metadata exclusions: ONLY the independently planned Fail+Rebind increments.
baseline['world']['revision']+=2;baseline['background']['incarnation']+=1;assert baseline==expected
assert load(B/'fault-reference/request.json')['retry_job']==clock['retry_job']
c=load(B/'world/CURRENT.json');raw=(B/'world'/f"root-{c['slot']}.json").read_bytes();assert sha(raw)==c['root_sha256'];root=json.loads(raw)
p=root['checkpoint_v2'];raw=(B/'world'/f"world-index-{p['generation']}.json").read_bytes();assert sha(raw)==p['sha256'];idx=json.loads(raw)
p=idx['current'];raw=(B/'world'/f"world-{p['generation']}.json").read_bytes();assert len(raw)==p['bytes'] and sha(raw)==p['sha256'] and json.loads(raw)==expected
ref=idx['receipts'];last=root['sequence'];operations=set()
while ref:
 raw=(B/'world'/f"receipt-index-{ref['generation']}.json").read_bytes();assert sha(raw)==ref['sha256'];i=json.loads(raw)
 raw=(B/'world'/f"receipts-{ref['generation']}.json").read_bytes();assert sha(raw)==i['body_sha256'] and len(raw)==i['body_bytes'];body=json.loads(raw)
 assert body['last']==i['last']==last and body['first']==i['first'] and body['previous']==i['previous'] and body['genesis_sha256']==root['genesis_sha256']
 assert len(body['receipts'])==last-i['first']+1 and [r['operation_id'] for r in body['receipts']]==i['operations']
 for receipt in body['receipts']:
  assert 'Ok' in receipt['outcome'] and receipt['operation_id'] not in operations;operations.add(receipt['operation_id'])
 last=i['first']-1;ref=i['previous']
assert last==0 and len(operations)==3013 and all((0xe000000000000000|job) in operations for job in range(1,725))
rows=[json.loads(s) for s in (B/'transcript.jsonl').read_text().splitlines()];data=[r['data'] for r in rows]
assert all(a['elapsed_seconds']<=b['elapsed_seconds'] for a,b in zip(rows,rows[1:]))
periodic={kind:[d for d in data if d.get('periodic')==kind] for kind in ['save','query']}
for kind,count,interval in [('save',96,300 if formal else .75),('query',2880,10 if formal else .025)]:
 ps=periodic[kind];assert len(ps)==count
 for n,d in enumerate(ps,1):
  assert d['ordinal']==n and abs(d['deadline_seconds']-n*interval)<1e-8 and d['deadline_seconds']<=d['submitted_seconds']<=d['completed_seconds']
  assert abs(d['completion_lateness_seconds']-(d['completed_seconds']-d['deadline_seconds']))<1e-6
hours=[d for d in data if 'hour_release' in d];assert len(hours)==720
for n,d in enumerate(hours,1):assert d['hour_release']==n and abs(d['deadline_seconds']-n*(40 if formal else .1))<1e-8 and d['submitted_seconds']>=d['deadline_seconds']
faults=[d for d in data if 'endurance_fault' in d];assert len(faults)==2
byfault={d['endurance_fault']:d for d in faults}
assert byfault['worker_cells']['job']==clock['retry_job'] and byfault['worker_cells']['at_seconds']>=(7200 if formal else 18)
assert byfault['sim_committed_reply']['job']==clock['committed_job'] and byfault['sim_committed_reply']['at_seconds']>=(14400 if formal else 36)
colds=[d for d in data if 'scheduled_reopen_deadline_seconds' in d];assert len(colds)==1 and colds[0]['submitted_seconds']>=colds[0]['scheduled_reopen_deadline_seconds']==(21600 if formal else 54)
verified=[d for d in data if 'endurance_verified_target' in d];assert len(verified)==723 and verified[-1]['endurance_verified_target']==2592000 and verified[-1]['elapsed_seconds']>=(28800 if formal else 72)
# Same entire normal CPU oracle and per-target hashes as the published v48 frozen run.
with zipfile.ZipFile(D.parent/'m0-t13a-broker/t13a3m-functional-01/artifacts.zip') as z:
 assert (B/'cpu-final-engine.json').read_bytes()==z.read('model/business/cpu-final-engine.json')
 assert (B/'cpu-targets.jsonl').read_bytes()==z.read('model/business/cpu-targets.jsonl')
pool=load(M/'normal-pool/report.json');s=pool['summary'];assert pool['balanced_after_owned_wait'] and s['grants']==s['returns']==s['fill_acknowledgements']==725 and s['rejected_requests']==0 and s['available_touched_bytes']==64*1024**2
heaps=[json.loads(s) for s in (M/'heaps.jsonl').read_text().splitlines()];assert all(h['success'] for h in heaps)
roles=collections.Counter(h['role'] for h in heaps)
assert roles=={'normal-pool-host':1,'scale-endurance':1,'scale-model-task':4,'sim-host-scale-full-probe':3,'compute-worker-bounded':724},roles
# Killed Sim/worker deliberately have NO normal final heap record; never infer zero.
normal_ids={(h['pid'],h['instance_unix_ns']) for h in heaps};producers=list((B/'diagnostics').glob('*/report.json'))
producer_dirs=[p for p in (B/'diagnostics').iterdir() if p.is_dir()]
assert len(producer_dirs)==729
missing_reports=[p for p in producer_dirs if not (p/'report.json').exists()]
assert collections.Counter(p.name.split('-')[0] for p in missing_reports)=={'sim':1,'worker':1}
for p in missing_reports:
 pid=int(p.name.split('-')[1]);assert not any(h['pid']==pid for h in heaps)
pool_rows=[json.loads(line) for line in (M/'normal-pool/events.jsonl').read_text().splitlines()]
active=None;session=None;ack=None;grants=0;sessions=0;waited=False;filled=False
for r in pool_rows:
 k=r['kind'];assert r['summary']['available_touched_bytes']+r['summary']['granted_capacity_bytes']==64*1024**2
 assert r['owner_actual_capacity_bytes']==r['summary']['available_touched_bytes']
 if k=='broker_session_registered':
  assert session is None and ack is None and active is None;session=r['broker_generation'];sessions+=1
 elif k=='broker_sim_bound':assert session==r['broker_generation'] and r['broker_sim_pid']>0
 elif k=='broker_session_revoked_after_sim_wait':
  assert session==r['broker_generation'] and active is None;ack=session;session=None
 elif k=='broker_revoke_receipt_acknowledged':assert session is None and ack==r['broker_generation'];ack=None
 elif k=='grant_released_before_spawn':
  assert active is None and session==r['broker_generation'];grants+=1;assert r['id']==grants;active=r['id'];waited=False;filled=False
 elif k=='worker_actual_slots_filled':assert active==r['id'] and not filled;filled=True
 elif k in ['owned_child_wait_success','owned_child_wait_failure']:assert active==r['id'] and filled and not waited;waited=True
 elif k=='returned_after_wait_or_abort':assert active==r['id'] and waited;active=None
assert session is None and ack is None and active is None and sessions==4 and grants==725
assert sum(d.get('kind')=='attempt_failed' for d in data)==1
assert len([d for d in data if d.get('command',{}).get('kind')=='probe_required'])==2
worker_candidates={}
for p in (B/'diagnostics').glob('worker-*/transactions.jsonl'):
 for line in p.read_text().splitlines():
  r=json.loads(line)
  if r.get('phase')!='worker_computation_complete':continue
  x=r['identity'];t=x['ticket'];key=(t['job'],t['start'],t['end'],t['field_version']);assert key not in worker_candidates
  worker_candidates[key]=(t['input_sha256'],t['checkpoint_sha256'],x['candidate_sha256'])
old_candidates={}
with zipfile.ZipFile(D.parent/'m0-t13a-broker/t13a3m-functional-01/artifacts.zip') as z:
 for name in z.namelist():
  if '/diagnostics/worker-' not in name or not name.endswith('/transactions.jsonl'):continue
  for line in z.read(name).splitlines():
   r=json.loads(line)
   if r.get('phase')!='worker_computation_complete':continue
   x=r['identity'];t=x['ticket'];key=(t['job'],t['start'],t['end'],t['field_version']);assert key not in old_candidates
   old_candidates[key]=(t['input_sha256'],t['checkpoint_sha256'],x['candidate_sha256'])
assert len(worker_candidates)==724 and worker_candidates==old_candidates

assert len(producers)==727
out={'source_sha256':info['source_sha256'],'scope':'original full workload/controller functional test; no L1','formal':formal,'clock':clock,'exact_final_engine_equal_fault_reference':True,'metadata_delta_from_normal':{'world.revision':2,'background.incarnation':1},'receipts':len(operations),'normal_heap_records':dict(roles),'deliberately_killed_processes_without_footer':{'Sim':1,'worker':1},'normal_diagnostic_producers':len(producers),'pool_grants_returned':s['returns'],'all_724_logical_input_candidate_hashes_equal_v48':True,'broker_sessions':sessions,'partial_producers_without_report':[p.name for p in missing_reports],'eight_hour':'not_accepted_without_joint_independent_audit' if formal else 'not_run','real_16GB':'not_run','full_M0':'not_accepted'}
print(json.dumps(out,indent=2))
