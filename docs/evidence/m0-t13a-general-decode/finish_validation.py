#!/usr/bin/env python3
"""Finish this exact regression; no benchmark/restart or cache directory creation."""
from pathlib import Path
import os,time,subprocess,json,hashlib,struct,zipfile
D=Path(__file__).resolve().parent;R=D.parents[2]
def identity(pid):
 p=subprocess.run(['ps','-p',str(pid),'-o','lstart='],stdout=subprocess.PIPE,text=True)
 return p.stdout.strip() if p.returncode==0 else None
pid=31262;started=identity(pid);assert started,'regression controller not live at launch'
(D/'waited-process.json').write_text(json.dumps(dict(pid=pid,lstart=started,scope='Wait existing exact full regression92234; never restart'),indent=2)+'\n')
while True:
 checks=json.loads((D/'checks-final.json').read_text()) if (D/'checks-final.json').exists() else {}
 if any(x['exit'] for x in checks.values()):raise SystemExit('regression failed')
 live=identity(pid)==started
 if set(checks)=={'workspace-release','clippy-final','fmt-final','build-final'} and not live:break
 if not live:raise SystemExit('terminal regression missing receipts')
 time.sleep(5)
info=json.loads((D/'freeze.json').read_text());h=hashlib.sha256()
with zipfile.ZipFile(D/'source.zip') as z:
 for n in sorted(z.namelist()):
  b=z.read(n);assert (R/n).read_bytes()==b;k=n.encode();h.update(struct.pack('<Q',len(k)));h.update(k);h.update(struct.pack('<Q',len(b)));h.update(b)
assert h.hexdigest()==info['source_sha256']
f=R/'target/release/mycity-headless';b=f.read_bytes();assert info['source_sha256'].encode() in b
fixed=R/'work/candidate-binaries/general-decode/mycity-headless';fixed.parent.mkdir(parents=True,exist_ok=True);os.link(f,fixed)
info.update(binary_sha256=hashlib.sha256(b).hexdigest(),binary=str(fixed.relative_to(R)),scope='Exact final full regression/build terminal0; bounded general decode implementation, no new joint/performance run')
(D/'source.json').write_text(json.dumps(info,indent=2)+'\n')
for name,cmd in [('capacity',[str(fixed),'model-capacity-plan']),('legacy',['python3',str(D/'verify_legacy.py')])]:
 t=time.monotonic()
 with (D/(name+'.stdout')).open('x') as out,(D/(name+'.stderr')).open('x') as err:p=subprocess.run(cmd,cwd=R,stdout=out,stderr=err)
 result=dict(command=cmd,exit=p.returncode,seconds=time.monotonic()-t);(D/(name+'-result.json')).write_text(json.dumps(result,indent=2)+'\n')
 print(name,p.returncode,flush=True)
 if p.returncode:raise SystemExit(p.returncode)
print('same-source regression, capacity and actual full legacy recovery terminal0',flush=True)
