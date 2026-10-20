#!/usr/bin/env python3
"""Report whole IPC calls; never sign compute gates using endpoint timings."""
import json,math,zipfile
from pathlib import Path
D=Path(__file__).resolve().parent
load=lambda p:json.loads(p.read_text())
def stats(values):
 s=sorted(values)
 return {'samples':len(s),'sum_seconds':sum(s),'p95_seconds':s[math.ceil(len(s)*.95)-1],'p99_seconds':s[math.ceil(len(s)*.99)-1],'maximum_seconds':s[-1]}
summary={}
for tier in ['small','medium','full']:
 out=D/tier;latencies={};pending=None;days=[];boundaries=[];last_day=0
 with zipfile.ZipFile(out/'measurements.zip') as z,z.open('model/transcript.jsonl') as f:
  for b in f:
   row=json.loads(b);v=row['data']
   if 'request' in v:
    assert pending is None;pending=(v['request'],v['command']['kind'])
   if v.get('kind')=='result':
    assert pending is not None and pending[0]==v['response'];latencies.setdefault(pending[1],[]).append(v['seconds']);pending=None
   if 'verified_target' in v:
    boundaries.append(v['boundary_seconds'])
    if v['verified_target']%86400==0:
     days.append({'game_day':v['verified_target']//86400,'elapsed_since_sim_ready_seconds':row['elapsed_seconds'],'since_previous_day_seconds':row['elapsed_seconds']-last_day});last_day=row['elapsed_seconds']
 assert pending is None and len(days)==30 and len(boundaries)==723
 report={'scope':'elapsed after submit returns through result processing; excludes request encode/submit; includes persistence and worker; not compute batch acceptance','ipc_calls':{k:stats(v) for k,v in latencies.items()},'complete_drive':stats(boundaries),'day_targets':days,'paced_speed_gates':'not_run','P_E0_30day_gate':'not_run'}
 (out/'ipc-diagnostic.json').write_text(json.dumps(report,indent=2)+'\n');summary[tier]={k:report[k] for k in ['ipc_calls','complete_drive']}
old=load(D.parent/'m0-t13a-full-sim/model.json');new=load(D/'full/model.json')
summary['full_single_diagnostic_comparison']={'before_sim_seconds':old['sim_work_wall_seconds_including_ipc_and_persistence'],'after_sim_seconds':new['sim_work_wall_seconds_including_ipc_and_persistence'],'samples_per_version':1,'same_frozen_full_input_and_complete_final_engine':'verified_separately_by_verify.py','formal_speed_gate':'not_run','not_a_controlled_three_run_benchmark':True}
(D/'ipc-summary.json').write_text(json.dumps(summary,indent=2)+'\n');print(json.dumps(summary['full_single_diagnostic_comparison'],indent=2))
