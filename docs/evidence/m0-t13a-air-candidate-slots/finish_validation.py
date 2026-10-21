#!/usr/bin/env python3
"""Wait only the existing live regression, then execute same-source proofs."""
import json,subprocess,time,hashlib,os
from pathlib import Path
D=Path(__file__).resolve().parent;R=D.parents[2]
pid=json.loads((D/'freeze.json').read_text())['controller_pid']
def identity():
 p=subprocess.run(['ps','-p',str(pid),'-o','lstart='],text=True,stdout=subprocess.PIPE)
 return p.stdout.strip() if p.returncode==0 else None
started=identity();assert started,'regression handle must be live at launch'
(D/'waited-process.json').write_text(json.dumps(dict(pid=pid,lstart=started,scope='Wait exact existing same existing regression; never restart on observation timeout'),indent=2)+'\n')
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
print('same-source regression and compatibility proofs terminal0',flush=True)
run=R/'work/t13a77-full-business-01'
assert not run.exists(),'never restart or overwrite a run'
env=os.environ.copy()
for key in ['MYCITY_NORMAL_POOL','MYCITY_BROKER_SESSION','MYCITY_BROKER_SUPERVISOR','MYCITY_NORMAL_WORKER_GRANT']:env.pop(key,None)
env['MYCITY_HEAP_LOG']=str(D/'full-rust-heaps.jsonl')
cmd=[str(binary),'scale-sim-probe',str(run),'full','measure']
t=time.monotonic()
with (D/'full-business.stdout').open('x') as out,(D/'full-business.stderr').open('x') as err:
 child=subprocess.Popen(cmd,cwd=R,env=env,stdout=out,stderr=err)
 (D/'full-business-running.json').write_text(json.dumps(dict(controller_pid=os.getpid(),pid=child.pid,command=cmd,source_sha256=info['source_sha256'],scope='Original full real30d business, no L1 or normal pool, functional diagnostic only'),indent=2)+'\n')
 code=child.wait()
result=dict(command=cmd,exit=code,seconds=time.monotonic()-t,run=str(run.relative_to(R)),source_sha256=info['source_sha256'],l1_pressure_included=False,normal_pool_authorization_accepted=False,performance_accepted=False,eight_hours_accepted=False,memory_16gb_gate='not_run')
(D/'full-business-result.json').write_text(json.dumps(result,indent=2)+'\n')
print('full-business',code,result['seconds'],flush=True)
if code:raise SystemExit(code)
report=json.loads((run/'report.json').read_text())
assert report['completed_hour_targets']==720 and report['verified_drive_targets']==723 and report['actual_worker_windows']==724
assert report['complete_final_business_and_air']=='passed' and report['oldest_receipt_retry']=='passed_without_side_effect'
assert report['performance_acceptance']=='not_run' and report['full_M0']=='not_accepted'
(D/'full-business-report.json').write_text(json.dumps(report,indent=2)+'\n')
print('full real30d functional assembly and original oracle passed; not L1/joint/performance acceptance',flush=True)
