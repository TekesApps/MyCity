#!/usr/bin/env python3
"""Continue only after this exact live regression finishes; never restart tests/runs."""
import hashlib,json,os,shutil,struct,subprocess,time,zipfile
from pathlib import Path
R=Path(__file__).resolve().parents[3];D=Path(__file__).resolve().parent
os.chdir(R);results={};source=json.loads((D/'freeze.json').read_text())
def save(): (D/'continuation-results.json').write_text(json.dumps(results,indent=2)+'\n')
def run(name,cmd,env=None):
 t=time.monotonic()
 with (D/(name+'.stdout')).open('x') as out,(D/(name+'.stderr')).open('x') as err:
  p=subprocess.run(cmd,stdout=out,stderr=err,env=env)
 results[name]={'command':list(map(str,cmd)),'exit':p.returncode,'seconds':time.monotonic()-t};save();print(name,p.returncode,flush=True)
 if p.returncode:raise SystemExit(p.returncode)
def identity(pid):
 p=subprocess.run(['ps','-p',str(pid),'-o','lstart='],stdout=subprocess.PIPE,text=True)
 return p.stdout.strip() if p.returncode==0 else None
pid=92037;started=identity(pid);assert started,'exact regression controller no longer live at launch'
results['regression_wait']={'pid':pid,'lstart':started,'required_keys':['workspace-release','clippy-final','fmt-final','build-final']};save()
while True:
 checks=json.loads((D/'checks-final.json').read_text()) if (D/'checks-final.json').exists() else {}
 if any(x['exit'] for x in checks.values()):raise SystemExit('regression failed; no follow-on run')
 live=identity(pid)==started
 if all(k in checks for k in results['regression_wait']['required_keys']) and not live:break
 if not live:raise SystemExit('regression controller exited without full terminal receipts')
 time.sleep(5)
h=hashlib.sha256()
with zipfile.ZipFile(D/'source.zip') as z:
 for n in sorted(z.namelist()):
  b=z.read(n);assert (R/n).read_bytes()==b,n;k=n.encode();h.update(struct.pack('<Q',len(k)));h.update(k);h.update(struct.pack('<Q',len(b)));h.update(b)
assert h.hexdigest()==source['source_sha256']
binary=R/'target/release/mycity-headless';raw=binary.read_bytes();assert source['source_sha256'].encode() in raw
fixed=R/'work/candidate-binaries/background-resident/mycity-headless';fixed.parent.mkdir(parents=True,exist_ok=True);os.link(binary,fixed)
info={**source,'binary_sha256':hashlib.sha256(raw).hexdigest(),'binary':str(fixed.relative_to(R)),'benchmark_binary_path':str(fixed.relative_to(R)),'frozen_acceptance_sha256':source['fixture_sha256'],'observer_sha256':hashlib.sha256((D/'observe.py').read_bytes()).hexdigest(),'scope':'Same-source full regression/build terminal0; immutable binary for original full joint600s, no performance acceptance implied'}
(D/'source.json').write_text(json.dumps(info,indent=2)+'\n')
# A model-free arithmetic report, not a runtime quota or physical-memory pass.
run('capacity',[str(fixed),'model-capacity-plan'])
pre=R/'work/t13a70-preflight';pre.mkdir();(pre/'measurement-requested').touch()
run('readiness',['python3',str(D/'readiness.py'),str(pre)])
# No source/build/cache work is started from this point until measurement terminal.
root=R/'work/t13a70-joint-01';assert not root.exists()
env=os.environ.copy();env['MYCITY_HEAP_LOG']=str(root/'model/heaps.jsonl')
results['measurement_launch']={'source':info['source_sha256'],'root_heap_log_environment':env['MYCITY_HEAP_LOG'],'load':'unchanged full L1+L2 smoke600s; NOT8h','readiness_exit':results['readiness']['exit']};save()
run('joint',['python3',str(D/'observe.py'),str(root),'--',str(fixed),'joint-endurance',str(root/'model'),'smoke'],env)
run('collect',['python3',str(D/'collect.py'),'t13a70-joint-01'])
for name in ['verify','workspace_audit','coverage','diagnostic_audit','latency_audit']:
 run(name,['python3',str(D/(name+'.py')),str(root),str(D)])
print('original full joint run and five independent audits terminal0',flush=True)
