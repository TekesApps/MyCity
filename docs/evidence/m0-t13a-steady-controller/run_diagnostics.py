"""Original-record finite controller diagnostic; NEVER accepts formal35min/8h.
No automatic retry or reduced fallback on any failure. Run only after regression.
"""
import hashlib,json,os,subprocess,time
from pathlib import Path
D=Path(__file__).resolve().parent;R=D.parents[2];os.chdir(R)
source=json.loads((D/'source.json').read_text());exe=Path(source['binary']);env=os.environ.copy()
for k in ['MYCITY_NORMAL_POOL','MYCITY_BROKER_SESSION','MYCITY_BROKER_SUPERVISOR','MYCITY_NORMAL_WORKER_GRANT','MYCITY_ROLE_CONSTRUCTION_OOM']:env.pop(k,None)
def run(name,cmd):
 assert hashlib.sha256(exe.read_bytes()).hexdigest()==source['binary_sha256']
 t=time.monotonic()
 with (D/(name+'.stdout')).open('x') as o,(D/(name+'.stderr')).open('x') as e:
  p=subprocess.Popen(cmd,env=env,stdout=o,stderr=e)
  (D/'diagnostic-running.json').write_text(json.dumps(dict(name=name,pid=p.pid,controller_pid=os.getpid(),command=cmd),indent=2)+'\n');code=p.wait()
 fact=dict(exit_code=code,seconds=time.monotonic()-t,command=cmd,source_sha256=source['source_sha256'])
 (D/(name+'.json')).write_text(json.dumps(fact,indent=2)+'\n');print(name,code,fact['seconds'],flush=True)
 if code:raise SystemExit(code)
for scenario in ['l0','l1','l2']:
 W=R/'work'/('steady-controller-smoke-'+scenario+'-01');assert not W.exists();W.mkdir();(W/'measurement-requested').touch()
 run(scenario+'-admission',['python3',str(D/'readiness.py'),str(W)])
 root=W/'observed'
 run(scenario+'-observed',['python3',str(D/'observe.py'),str(root),'--',str(exe),'steady-run',str(root/'model'),scenario,'smoke'])
 run(scenario+'-window-audit',['python3',str(D/'window_audit.py'),str(root/'model')])
 run(scenario+'-resource-window-audit',['python3',str(D/'resource_window_audit.py'),str(root)])
 report=json.loads((root/'model/business/report.json').read_text())
 assert report['formal'] is False
 if scenario=='l2':
  b=root/'model/business/measurement';business=json.loads((b/'report.json').read_text())
  assert business['completed_hours']==720 and business['verified_targets']==723 and business['worker_windows']==724
  assert business['entire_engine_including_metadata']=='equal_independent_normal_reference'
  assert (b/'cpu-targets.jsonl').read_bytes()==(R/'work/t13a-role-arena-business-01/business/cpu-targets.jsonl').read_bytes()
