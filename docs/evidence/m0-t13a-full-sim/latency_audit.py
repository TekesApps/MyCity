#!/usr/bin/env python3
"""Whole IPC calls and target durations, never the ten compute-class acceptance gates."""
import json,math,zipfile
from pathlib import Path
D=Path(__file__).resolve().parent
latencies={};pending=None;days=[];boundaries=[];last_day=0
with zipfile.ZipFile(D/'measurements.zip') as z,z.open('model/transcript.jsonl') as f:
 for b in f:
  row=json.loads(b);v=row['data']
  if 'request' in v:
   assert pending is None
   pending=(v['request'],v['command']['kind'])
  if v.get('kind')=='result':
   assert pending is not None and pending[0]==v['response']
   latencies.setdefault(pending[1],[]).append(v['seconds']);pending=None
  if 'verified_target' in v:
   boundaries.append(v['boundary_seconds'])
   if v['verified_target']%86400==0:
    days.append({'game_day':v['verified_target']//86400,'elapsed_since_sim_ready_seconds':row['elapsed_seconds'],'since_previous_day_seconds':row['elapsed_seconds']-last_day})
    last_day=row['elapsed_seconds']
assert pending is None and len(days)==30 and len(boundaries)==723
def stats(values):
 s=sorted(values)
 return {'samples':len(s),'sum_seconds':sum(s),'p95_seconds':s[math.ceil(len(s)*.95)-1],'p99_seconds':s[math.ceil(len(s)*.99)-1],'maximum_seconds':s[-1]}
out={'scope':'whole IPC round trips including storage and worker; not CPU compute-class gates','ipc_calls':{k:stats(v) for k,v in latencies.items()},'complete_drive':stats(boundaries),'actual_day_completion_targets':days,'paced_speed_gates':'not_run','P_E0_30day_gate':'not_run','eight_hour_gate':'not_run'}
(D/'ipc-diagnostic.json').write_text(json.dumps(out,indent=2)+'\n');print(json.dumps({k:v for k,v in out.items() if k!='actual_day_completion_targets'},indent=2))
