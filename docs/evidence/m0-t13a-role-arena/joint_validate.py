"""One exact-source joint smoke diagnostic after full business+boundary terminal.
No build or cleanup during measurement; no restart and no load reduction.
"""
import hashlib,json,os,struct,subprocess,time,zipfile
from pathlib import Path
D=Path(__file__).resolve().parent;R=D.parents[2];os.chdir(R)
load=lambda p:json.loads(p.read_text())
info=load(D/'arena-source.json');assert load(D/'arena-terminal.json')['functional_original_full_30day_verified']
assert load(D/'store-boundary-proof.json')['synthetic_world_and_base_unchanged']
def check():
 h=hashlib.sha256()
 with zipfile.ZipFile(D/'arena-source.zip') as z:
  for n in sorted(z.namelist()):
   b=z.read(n);assert (R/n).read_bytes()==b,n;k=n.encode();h.update(struct.pack('<Q',len(k)));h.update(k);h.update(struct.pack('<Q',len(b)));h.update(b)
 assert h.hexdigest()==info['source_sha256']
 assert hashlib.sha256((R/info['binary']).read_bytes()).hexdigest()==info['binary_sha256']
def save(n,v):(D/n).write_text(json.dumps(v,indent=2)+'\n')
def run(n,cmd,env=None):
 check();t=time.monotonic()
 with (D/(n+'.stdout')).open('x') as o,(D/(n+'.stderr')).open('x') as e:
  p=subprocess.Popen(cmd,cwd=R,env=env,stdout=o,stderr=e)
  save('joint-running.json',dict(controller_pid=os.getpid(),pid=p.pid,name=n,command=cmd))
  code=p.wait()
 fact=dict(command=cmd,exit_code=code,seconds=time.monotonic()-t);save(n+'.json',fact);print(n,code,fact['seconds'],flush=True);check()
 if code:raise SystemExit(code)
check();save('source.json',info)
pre=R/'work/t13a85-preflight';assert not pre.exists();pre.mkdir();(pre/'measurement-requested').touch()
run('joint-readiness',['python3',str(D/'readiness.py'),str(pre)])
root=R/'work/t13a85-joint-01';assert not root.exists()
env=os.environ.copy()
for k in ['MYCITY_NORMAL_POOL','MYCITY_BROKER_SESSION','MYCITY_BROKER_SUPERVISOR','MYCITY_NORMAL_WORKER_GRANT','MYCITY_ROLE_CONSTRUCTION_OOM']:env.pop(k,None)
env['MYCITY_HEAP_LOG']=str(root/'model/heaps.jsonl')
run('joint-measurement',['python3',str(D/'observe.py'),str(root),'--',str(R/info['binary']),'joint-endurance',str(root/'model'),'smoke'],env)
run('joint-collect',['python3',str(D/'collect.py'),root.name])
for name in ['verify','workspace_audit','coverage','diagnostic_audit','latency_audit']:
 run('joint-'+name,['python3',str(D/(name+'.py')),str(root),str(D)])
save('joint-terminal.json',dict(source_sha256=info['source_sha256'],same_source_full_joint_smoke_completed=True,formal_steady='not_run',four_speeds='not_run',eight_hour='not_run',real_16GB='not_run',full_M0='not_accepted'))
