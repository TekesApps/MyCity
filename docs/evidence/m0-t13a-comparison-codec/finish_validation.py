#!/usr/bin/env python3
"""Wait only the existing live regression, then execute same-source proofs."""
import json,subprocess,time,hashlib
from pathlib import Path
D=Path(__file__).resolve().parent;R=D.parents[2]
pid=json.loads((D/'freeze.json').read_text())['controller_pid']
def identity():
 p=subprocess.run(['ps','-p',str(pid),'-o','lstart='],text=True,stdout=subprocess.PIPE)
 return p.stdout.strip() if p.returncode==0 else None
started=identity();assert started,'regression handle must be live at launch'
(D/'waited-process.json').write_text(json.dumps(dict(pid=pid,lstart=started,scope='Wait exact existing regression27121; never restart on observation timeout'),indent=2)+'\n')
while True:
 checks=json.loads((D/'checks-final.json').read_text()) if (D/'checks-final.json').exists() else {}
 if any(x['exit'] for x in checks.values()):raise SystemExit('final regression failure')
 live=identity()==started
 if not live:
  assert set(checks)=={'workspace-release','clippy-final','fmt-final','build-final'} and (D/'source.json').exists(),'terminal process without complete receipts'
  break
 time.sleep(5)
info=json.loads((D/'source.json').read_text());binary=R/info['binary']
assert hashlib.sha256(binary.read_bytes()).hexdigest()==info['binary_sha256']
for name,cmd in [('capacity',[str(binary),'model-capacity-plan']),('legacy',['python3',str(D/'verify_legacy.py')]),('codec',['python3',str(D/'verify_codec.py')])]:
 t=time.monotonic()
 with (D/(name+'-check.stdout')).open('x') as out,(D/(name+'-check.stderr')).open('x') as err:
  code=subprocess.run(cmd,cwd=R,stdout=out,stderr=err).returncode
 (D/(name+'-result.json')).write_text(json.dumps(dict(command=cmd,exit=code,seconds=time.monotonic()-t),indent=2)+'\n')
 print(name,code,flush=True)
 if code:raise SystemExit(code)
print('same-source final regression and full old checkpoint and comparison byte proofs terminal0',flush=True)
