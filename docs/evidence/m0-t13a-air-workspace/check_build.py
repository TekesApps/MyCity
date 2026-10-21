import json,subprocess,time
from pathlib import Path
D=Path(__file__).resolve().parent
results=[]
for name,cmd in [
 ('clippy',['cargo','clippy','--workspace','--all-targets','--release','--locked','--offline','--','-D','warnings']),
 ('fmt',['cargo','fmt','--all','--','--check']),
 ('build',['cargo','build','--workspace','--release','--locked','--offline']),
]:
 begin=time.monotonic()
 with (D/(name+'.txt')).open('x') as f:r=subprocess.run(cmd,stdout=f,stderr=subprocess.STDOUT)
 results.append({'name':name,'command':cmd,'exit_code':r.returncode,'wall_seconds':time.monotonic()-begin})
 (D/'build-checks.json').write_text(json.dumps(results,indent=2)+'\n')
 print(name,r.returncode,flush=True)
 if r.returncode:raise SystemExit(r.returncode)
