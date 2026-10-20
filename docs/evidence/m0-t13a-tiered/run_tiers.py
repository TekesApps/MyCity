#!/usr/bin/env python3
"""Run each frozen tier once, sequentially; no retries or hidden restarts."""
import json,subprocess,sys,time
from pathlib import Path
R=Path(__file__).resolve().parents[3]
base=R/'work'/sys.argv[1];base.mkdir()
rows=[]
for tier in ['small','medium','full']:
 run=base/tier;start=time.monotonic()
 command=[sys.executable,str(R/'docs/evidence/m0-t13a-layout/observe.py'),str(run),'--artifacts-root',str(base),'--',str(R/'target/release/mycity-headless'),'scale-sim-probe',str(run/'model'),tier]
 print('starting '+tier,flush=True)
 with (base/(tier+'-observer.stdout')).open('wb') as f:
  result=subprocess.run(command,cwd=R,stdout=f,stderr=subprocess.STDOUT)
 rows.append({'tier':tier,'exit_code':result.returncode,'wall_seconds':time.monotonic()-start,'command':command})
 (base/'execution.json').write_text(json.dumps(rows,indent=2)+'\n')
 if result.returncode:raise SystemExit(result.returncode)
 print('completed '+tier,flush=True)
