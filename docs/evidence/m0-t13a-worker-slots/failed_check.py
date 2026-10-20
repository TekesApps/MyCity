#!/usr/bin/env python3
import collections,json,sys,zipfile
from pathlib import Path
R=Path(sys.argv[1]);M=R/'model';load=lambda p:json.loads(p.read_text());o=load(R/'observer-report.json')
assert o['status']=='failed' and o['reason']=='system non-normal pressure exceeded 60 seconds' and o['maximum_sampled_non_normal_pressure_seconds']>60 and o['model_exit_code']<0
heap={}
for line in (M/'heaps.jsonl').read_text().splitlines():
 v=json.loads(line);k=(v['pid'],v['instance_unix_ns']);assert k not in heap and v['success'] and v['role']=='compute-worker-bounded';heap[k]=v
reports={};partial=[]
for p in (M/'diagnostics').glob('*/report.json'):
 try:v=load(p)
 except json.JSONDecodeError:partial.append(p.parent.name);continue
 if v['producer']!='worker':continue
 reports[(v['process_pid'],v['process_instance_unix_ns'])]=(p,v)
assert heap.keys()<=reports.keys()
old={}
with zipfile.ZipFile(Path(__file__).resolve().parents[1]/'m0-t13a-process-instances/t13a3c-instances-01/artifacts.zip') as z:
 for name in z.namelist():
  if '/diagnostics/worker-' not in name or not name.endswith('/transactions.jsonl'):continue
  for line in z.read(name).splitlines():
   row=json.loads(line)
   if row.get('phase')=='worker_computation_complete':
    v=row['identity'];t=v['ticket'];key=tuple(t[n] for n in ['job','incarnation','generation','attempt','start','end','field_version']);assert key not in old
    old[key]=(t['input_sha256'],t['checkpoint_sha256'],v['candidate_sha256'])
for k in heap:
 p,d=reports[k];assert d['external_diagnostics_owned_bytes']>0 and d['charged_diagnostics_bound_bytes']<=1024**2
 owners=[];completed=[]
 for line in (p.parent/'transactions.jsonl').read_text().splitlines():
  row=json.loads(line)
  if row.get('phase')=='worker_controlled_slots':owners.append(row['owner'])
  if row.get('phase')=='worker_computation_complete':completed.append(row['identity'])
 assert len(owners)==len(completed)==1;r=owners[0]
 t=completed[0]['ticket'];key=tuple(t[n] for n in ['job','incarnation','generation','attempt','start','end','field_version'])
 assert old[key]==(t['input_sha256'],t['checkpoint_sha256'],completed[0]['candidate_sha256'])
 assert (r['identity']['process_pid'],r['identity']['process_instance_unix_ns'])==k
 assert r['owner_capacity_bytes']==r['available_touched_bytes']==4*1024**2 and r['lent_capacity_bytes']==0 and r['full_balance_scan_passed']
 assert r['controlled_metadata_charge_bytes']==d['external_diagnostics_owned_bytes'] and len(r['transitions'])==4
 for n,slot in enumerate(r['slots']):
  assert slot['capacity_bytes']==slot['available_touched_bytes']==2*1024**2 and slot['claims']==slot['returns']==1
  a,b=r['transitions'][n*2:n*2+2];assert a['ticket']==b['ticket'] and a['borrowed'] and not b['borrowed'] and b['work_succeeded']
frames=0
for line in (R/'host-samples.jsonl').read_text().splitlines():
 row=json.loads(line);assert row['rss_bytes']==sum(v[3] for v in row['members']);frames+=1
print(json.dumps({'partial_worker_hash_pairs_equal_v42':len(heap),'RSS_frames_checked':frames,'status' :'pressure_failure_retained','wall_seconds':o['actual_wall_seconds'],'maximum_non_normal_pressure_seconds':o['maximum_sampled_non_normal_pressure_seconds'],'complete_worker_exit_records':len(heap),'paired_complete_local_owners':len(heap),'sealed_worker_reports':len(reports),'partial_report_files':partial,'missing_parent_or_active_process_records':'SIGTERM leaves unmeasured records, never counted as zero','full_30days':'not_completed','formal_gate':'failed'},indent=2))
