#!/usr/bin/env python3
"""Maintain the last 25 live pressure samples; release only after caller's start flag."""
import collections,json,subprocess,sys,time
from pathlib import Path
W=Path(sys.argv[1]);W.mkdir(exist_ok=True);rows=collections.deque(maxlen=25);begin=time.monotonic()
with (W/'pressure-preflight.jsonl').open('x') as f:
 for i in range(181):
  level=int(subprocess.check_output(['/usr/sbin/sysctl','-n','kern.memorystatus_vm_pressure_level']))
  row={'elapsed_seconds':time.monotonic()-begin,'pressure':level,'unix_ns':time.time_ns()};rows.append(row);f.write(json.dumps(row)+'\n');f.flush()
  (W/'pressure-latest.json').write_text(json.dumps(row)+'\n')
  if (W/'measurement-requested').exists() and len(rows)==25 and all(r['pressure']==1 for r in rows) and rows[-1]['elapsed_seconds']-rows[0]['elapsed_seconds']>=120:
   (W/'readiness.json').write_text(json.dumps({'samples':list(rows)},indent=2)+'\n');print('normal120_ready',row['elapsed_seconds']);break
  if i==180:raise SystemExit('normal120_or_measurement_request_not_obtained_within_900s')
  time.sleep(5)
