"""Actual original L2 formal round1: 300s warmup+1800s measurement. No retry.
Each acceptance axis audited separately; exit0 never means all gates passed.
"""
import hashlib,json,os,struct,subprocess,time,zipfile
from pathlib import Path
D=Path(__file__).resolve().parent;R=D.parents[2];os.chdir(R);info=json.loads((D/'source.json').read_text());exe=Path(info['binary']);W=R/'work/steady-controller-formal-l2-01'
assert not W.exists()
def check_source():
 h=hashlib.sha256()
 with zipfile.ZipFile(D/'frozen-source.zip') as z:
  for n in sorted(z.namelist()):
   b=z.read(n);assert (R/n).read_bytes()==b,n;s=n.encode();h.update(struct.pack('<Q',len(s)));h.update(s);h.update(struct.pack('<Q',len(b)));h.update(b)
 assert h.hexdigest()==info['source_sha256'];assert hashlib.sha256(exe.read_bytes()).hexdigest()==info['binary_sha256']
check_source();env=os.environ.copy()
for k in ['MYCITY_NORMAL_POOL','MYCITY_BROKER_SESSION','MYCITY_BROKER_SUPERVISOR','MYCITY_NORMAL_WORKER_GRANT','MYCITY_ROLE_CONSTRUCTION_OOM']:env.pop(k,None)
def run(name,cmd):
 check_source();started=time.monotonic()
 with (D/(name+'.stdout')).open('x') as o,(D/(name+'.stderr')).open('x') as e:
  p=subprocess.Popen(cmd,env=env,stdout=o,stderr=e)
  (D/'formal-running.json').write_text(json.dumps(dict(controller_pid=os.getpid(),pid=p.pid,name=name,command=cmd,source_sha256=info['source_sha256']),indent=2)+'\n');code=p.wait()
 fact=dict(exit_code=code,seconds=time.monotonic()-started,command=cmd,source_sha256=info['source_sha256']);(D/(name+'.json')).write_text(json.dumps(fact,indent=2)+'\n');print(name,code,fact['seconds'],flush=True);check_source()
 if code:raise SystemExit(code)
W.mkdir();(W/'measurement-requested').touch()
run('formal-l2-01-admission',['python3',str(D/'readiness.py'),str(W)])
root=W/'observed';run('formal-l2-01-observed',['python3',str(D/'observe.py'),str(root),'--',str(exe),'steady-run',str(root/'model'),'l2','formal'])
for name,script,arg in [('window-audit','window_audit.py',root/'model'),('resource-window-audit','resource_window_audit.py',root),('business-audit','business_audit.py',root)]:
 run('formal-l2-01-'+name,['python3',str(D/script),str(arg)])
(D/'formal-l2-01-terminal.json').write_text(json.dumps(dict(source_sha256=info['source_sha256'],actual_round_completed=True,formal_steady='requires_axis_audits_and_independent_review',three_rounds='not_accepted_by_single_round',eight_hour='not_run',real_16GB='not_run',full_M0='not_accepted'),indent=2)+'\n')
