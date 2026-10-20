#!/usr/bin/env python3
"""Original full business and exact operating-clock audit. Not a joint/16GB claim."""
import collections,hashlib,json,math,sys,zipfile
from pathlib import Path
D=Path(__file__).resolve().parent;R=Path(sys.argv[1]);M=R/'model';B=M/'business';load=lambda p:json.loads(p.read_text());sha=lambda b:hashlib.sha256(b).hexdigest();info=load(D/'source.json')
o=load(R/'observer-report.json');parent=load(M/'report.json');report=load(B/'report.json');manifest=load(B/'speed-manifest.json');rate=report['rate'];day=None if rate=='unpaced' else int(rate)
assert rate in ['unpaced','600','60','10','1'] and rate==parent['rate']==manifest['rate']
assert o['status']=='passed_measurement_wrapper' and o['binary_unchanged'] and o['observer_source_unchanged']
assert o['binary_sha256']==info['binary_sha256'] and parent['source_sha256']==report['source_sha256']==manifest['source_sha256']==info['source_sha256']
assert parent['experiment_completed'] and report['completed_hours']==720 and report['verified_targets']==723 and report['worker_windows']==724
assert report['profile']=='L2_full_actual' and report['mandatory_pe0_gate']=='not_evaluated_other_profile' and report['legacy_session_speed']=='not_retested'
assert report['entire_engine_including_metadata']=='equal_independent_normal_reference'
with zipfile.ZipFile(D.parent/'m0-t13a-broker/t13a3m-functional-01/artifacts.zip') as z:
 assert (B/'cpu-final-engine.json').read_bytes()==z.read('model/business/cpu-final-engine.json')
 assert (B/'cpu-targets.jsonl').read_bytes()==z.read('model/business/cpu-targets.jsonl')
# Bind stored full Engine, including metadata, to the independent normal CPU.
world=B/'world';cur=load(world/'CURRENT.json');raw=(world/f"root-{cur['slot']}.json").read_bytes();assert sha(raw)==cur['root_sha256'];root=json.loads(raw)
p=root['checkpoint_v2'];raw=(world/f"world-index-{p['generation']}.json").read_bytes();assert sha(raw)==p['sha256'];index=json.loads(raw)
p=index['current'];raw=(world/f"world-{p['generation']}.json").read_bytes();assert len(raw)==p['bytes'] and sha(raw)==p['sha256'];assert json.loads(raw)==load(B/'cpu-final-engine.json')
proof=load(B/'compare-speed/result.json');before=load(B/'before-reopen/result.json');assert proof['image_sha256']==before['image_sha256']==sha(raw)
ref=index['receipts'];last=root['sequence'];operations=set()
while ref:
 raw=(world/f"receipt-index-{ref['generation']}.json").read_bytes();assert sha(raw)==ref['sha256'];i=json.loads(raw)
 raw=(world/f"receipts-{ref['generation']}.json").read_bytes();assert sha(raw)==i['body_sha256'] and len(raw)==i['body_bytes'];body=json.loads(raw)
 assert body['last']==i['last']==last and body['first']==i['first'] and body['previous']==i['previous'] and body['genesis_sha256']==root['genesis_sha256']
 assert len(body['receipts'])==last-i['first']+1 and [r['operation_id'] for r in body['receipts']]==i['operations']
 for r in body['receipts']:assert 'Ok' in r['outcome'] and r['operation_id'] not in operations;operations.add(r['operation_id'])
 last=i['first']-1;ref=i['previous']
assert last==0 and len(operations)==3011 and all((0xe000000000000000|n) in operations for n in range(1,725))
rows=[json.loads(l) for l in (B/'transcript.jsonl').open()];assert all(a['elapsed_seconds']<=b['elapsed_seconds'] for a,b in zip(rows,rows[1:]));data=[r['data'] for r in rows]
assert any(x.get('speed_operating_started_before_sim_create') is True for x in data)
start=next(x for x in data if x.get('speed_operating_started_before_sim_create'))
assert report['sim_ready_seconds']==start['sim_ready_seconds']>0
submitted=[x for x in data if 'speed_target_submitted' in x];completed=[x for x in data if 'speed_target_completed' in x];verified=[x for x in data if 'verified_target' in x]
assert len(submitted)==len(completed)==len(verified)==723
reference=[json.loads(l) for l in (B/'cpu-targets.jsonl').open()]
for s,c,v,expected in zip(submitted,completed,verified,reference):
 t=expected['target'];release=0 if day is None else (day*1000000000*(math.ceil(t/3600)-1)//24)/1e9
 assert s['speed_target_submitted']==c['speed_target_completed']==v['verified_target']==t
 assert s['release_seconds']==release and s['submitted_seconds']>=release
 assert c['completed_seconds']>=s['submitted_seconds']
 assert abs(s['nominal_release_age_seconds']-(s['submitted_seconds']-release))<1e-8
 assert v['expected']==expected and v['actual_business_sha256']==expected['business_sha256'] and v['actual_air_sha256']==expected['air_sha256']
# Each cutoff binds the actual final checkpoint request/result in the transcript.
cutoff=report['publication_end_seconds'];end=report['operating_wall_seconds'];assert report['business_targets_complete_seconds']<=cutoff==report['final_checkpoint_completed_seconds']<=end
ends=[(i,x) for i,x in enumerate(data) if 'speed_operating_end_seconds' in x];assert len(ends)==1;end_index,endrow=ends[0];assert endrow['speed_operating_end_seconds']==end and endrow['publication_end_seconds']==cutoff
markers=[(i,x) for i,x in enumerate(data) if 'speed_final_business_checkpoint' in x];assert len(markers)==1
marker_index,marker=markers[0];request=report['final_checkpoint_request_id']
assert marker['speed_final_business_checkpoint']==request and marker['publication_frozen']
assert marker['submitted_seconds']==report['final_checkpoint_submitted_seconds']<=marker['completed_seconds']==cutoff
reqs=[(i,x) for i,x in enumerate(data) if x.get('request')==request and x.get('command',{}).get('kind')=='checkpoint'];assert len(reqs)==1
req_index,_=reqs[0];results=[i for i,x in enumerate(data) if x.get('response')==request and x.get('kind')=='result'];assert len(results)==1
last_target_index=max(i for i,x in enumerate(data) if 'speed_target_completed' in x)
assert last_target_index<req_index<results[0]<marker_index<end_index
assert not any(x.get('command',{}).get('kind') in ['execute','run_required'] for x in data[marker_index+1:end_index])
for kind,interval,field,nextfield in [('query',10,'periodic_queries_completed','next_unpublished_query_deadline_seconds'),('save',300,'periodic_saves_completed','next_unpublished_save_deadline_seconds')]:
 ps=[x for x in data if x.get('speed_periodic')==kind];expected_count=math.floor(cutoff/interval)
 assert len(ps)==report[field]==expected_count
 for n,x in enumerate(ps,1):
  assert x['ordinal']==n and x['deadline_seconds']==n*interval<=cutoff
  assert x['deadline_seconds']<=x['submitted_seconds']<=x['completed_seconds']<=end
  assert abs(x['completion_lateness_seconds']-(x['completed_seconds']-x['deadline_seconds']))<1e-8
 assert report[nextfield]==(expected_count+1)*interval>cutoff
 # Future deadlines during the drain remain UNPUBLISHED, not silently completed.
 assert all(x['deadline_seconds']<=cutoff for x in ps)
days=report['days'];assert len(days)==30;previous=0;day_met=[]
for n,d in enumerate(days,1):
 assert d['day']==n
 target=n*86400;c=next(c for c in completed if c['speed_target_completed']==target)
 assert d['business_durable_seconds']>=c['completed_seconds']
 assert abs(d['adjacent_business_completion_seconds']-(d['business_durable_seconds']-previous))<1e-8
 if day is None:
  assert d['fixed_period_start_seconds'] is None and d['fixed_period_to_business_seconds'] is None;elapsed=d['adjacent_business_completion_seconds']
 else:
  assert d['fixed_period_start_seconds']==(n-1)*day
  elapsed=d['business_durable_seconds']-(n-1)*day
  assert abs(d['fixed_period_to_business_seconds']-elapsed)<1e-8
 met=elapsed<=(day or 60);assert d['budget_met']==met;day_met.append(met);previous=d['business_durable_seconds']
last_start=days[28]['business_durable_seconds'] if day is None else 29*day
assert abs(report['final_day_with_checkpoint_seconds']-(end-last_start))<1e-8
all_days=all(day_met) and end-last_start<=(day or 60);total=end<=30*(day or 60);met=all_days and total
assert report['all_daily_budgets_met']==all_days and report['whole_30day_budget_met']==total and report['speed_target_met']==met
pool=load(M/'normal-pool/report.json');summary=pool['summary'];assert pool['balanced_after_owned_wait'] and summary['grants']==summary['returns']==summary['fill_acknowledgements']==724 and summary['available_touched_bytes']==64*1024**2 and summary['granted_capacity_bytes']==0
heaps=[json.loads(l) for l in (M/'heaps.jsonl').open()];assert all(x['success'] and x['source_sha256']==info['source_sha256'] for x in heaps)
normal={(x['pid'],x['instance_unix_ns']) for x in heaps};assert len(normal)==len(heaps)
roles=collections.Counter(x['role'] for x in heaps);assert roles=={'speed-run':1,'speed-driver':1,'normal-pool-host':1,'scale-model-task':3,'sim-host-scale-full':2,'compute-worker-bounded':724},roles
ids=set();reports=list((B/'diagnostics').glob('*/report.json'));assert len(reports)==726
for p in (B/'diagnostics').iterdir():
 if not p.is_dir():continue
 with (p/'transactions.jsonl').open('rb') as f:line=f.readline(1025)
 assert len(line)<=1024 and line.endswith(b'\n');r=json.loads(line);assert r['phase']=='producer_identity';identity=r['identity'];k=(identity['process_pid'],identity['process_instance_unix_ns']);assert k in normal and k not in ids;ids.add(k)
 assert p.name==f"{identity['producer']}-{identity['process_pid']}-{identity['producer_instance']}"
 sealed=load(p/'report.json');assert all(sealed[n]==identity[n] for n in ['producer','producer_instance','process_pid','process_instance_unix_ns','source_sha256'])
 assert sealed['transaction_log_bytes']==(p/'transactions.jsonl').stat().st_size
for h in heaps:
 if h['role'] in ['speed-run','speed-driver']:
  cap=(1024**2//4) if h['role']=='speed-run' else 1024**2
  assert h['heap']['lifetime_peak_requested_bytes']+h['tail_reserved_bytes']<=cap
print(json.dumps({'source_sha256':info['source_sha256'],'rate':rate,'profile':'L2_full_actual','mandatory_pe0_gate':'not_evaluated_other_profile','legacy_session_speed':'not_retested','experiment_completed':True,'operating_seconds':end,'publication_cutoff_seconds':cutoff,'queries':report['periodic_queries_completed'],'saves':report['periodic_saves_completed'],'all_daily_budgets_met':all_days,'whole_budget_met':total,'speed_target_met':met,'all_723_targets_equal_original_reference':True,'exact_complete_Engine_including_metadata':True,'original_receipts':len(operations),'normal_heap_records':dict(roles),'L1':'absent','common_8MiB':'not_accepted','normal_3_10GiB':'not_accepted','eight_hour':'not_run','real_16GB':'not_run','full_M0':'not_accepted'},indent=2))
