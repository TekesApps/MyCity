#!/usr/bin/env python3
"""One run per branch, sequential; retain errors and never retry a measurement."""
import json,subprocess,sys,time,os
from pathlib import Path
R=Path(__file__).resolve().parents[3];D=Path(__file__).resolve().parent
rows=[]
for name,args,measured in [
 ('t13a2e-full-02',['scale-sim-probe','full','measure'],False),
 ('t13a2e-l0-01',['economy-probe','fixtures/m0-economy-opening.json'],True)]:
 run=R/'work'/name;binary=R/'target/release/mycity-headless'
 cli=([str(binary),args[0],str(run/'model')]+args[1:]) if 'full' in name else [str(binary)]+args+[str(run/'model')]
 command=[sys.executable,str(D/'observe_v2.py'),str(run),'--']+cli
 env=os.environ.copy()
 if measured:env['MYCITY_DIAGNOSTICS']='1'
 else:env.pop('MYCITY_DIAGNOSTICS',None)
 start=time.monotonic();print('starting '+name,flush=True)
 with (R/'work'/(name+'-observer.stdout')).open('wb') as f:
  p=subprocess.run(command,cwd=R,env=env,stdout=f,stderr=subprocess.STDOUT)
 rows.append({'name':name,'command':command,'MYCITY_DIAGNOSTICS':env.get('MYCITY_DIAGNOSTICS'),'exit_code':p.returncode,'wall_seconds':time.monotonic()-start})
 (D/'execution.json').write_text(json.dumps(rows,indent=2)+'\n')
 print('completed '+name+' exit='+str(p.returncode),flush=True)
 if p.returncode:raise SystemExit(p.returncode)
