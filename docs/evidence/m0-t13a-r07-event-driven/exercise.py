"""Full original L1, actual64MiB pool, two denser diagnostic snapshots.
Pause only provider publication to invalidate TTL; sampler and safety remain live.
This is NOT a formal performance run; snapshot schedule is diagnostic event-driven.
"""
import ctypes,hashlib,importlib.util,json,os,signal,subprocess,sys,time,zipfile
from pathlib import Path
D=Path(__file__).resolve().parent;R=D.parents[2];os.chdir(R)
P=D.parent/'m0-t13a-typed-workspace';sys.path.insert(0,str(P))
spec=importlib.util.spec_from_file_location('r07sdk',P/'observe.py');sdk=importlib.util.module_from_spec(spec);spec.loader.exec_module(sdk)
info=json.loads((D/'source.json').read_text());exe=info['binary'];assert hashlib.sha256(Path(exe).read_bytes()).hexdigest()==info['binary_sha256']
with zipfile.ZipFile(D/'frozen-source.zip') as z:
 for n in z.namelist():assert (R/n).read_bytes()==z.read(n),n
W=R/'work/r07-event-driven-l1-01';assert not W.exists();W.mkdir();root=W/'observed';cmd=[exe,'native-observe',str(root),'--',exe,'r07-layout-driver',str(root/'model')]
assert os.statvfs(R).f_bavail*os.statvfs(R).f_frsize>13*1024**3
(W/'intent.json').write_text(json.dumps(dict(source_sha256=info['source_sha256'],binary_sha256=info['binary_sha256'],purpose='full L1 pin/TTL/partial snapshot/cold protocol only; provider-only pause, no formal performance claim',original_record_counts=True,actual_pool_bytes=64*1024**2,estimated_peak_bytes=11*1024**3,retention='payload eligible only after all cold validation and independent dependency release; keep epoch/raw/typed/index and protected originals'),indent=2)+'\n')
def read(p):
 try:return json.loads(p.read_text())
 except FileNotFoundError:return None

def events():
 p=root/'model/layout/events.jsonl'
 return [json.loads(x) for x in p.read_text().splitlines() if x.endswith('}')] if p.exists() else []
def waitfor(fn,p,limit=60):
 end=time.monotonic()+limit
 while time.monotonic()<end:
  v=fn()
  if v:return v
  if p.poll() is not None:raise RuntimeError('existing wrapper terminal before expected condition; inspect its report, never restart')
  time.sleep(.005)
 raise RuntimeError('observation timeout; inspect existing live handle, do not restart')
start=time.monotonic();facts=[]
with (D/'exercise.stdout').open('x') as o,(D/'exercise.stderr').open('x') as e:
 p=subprocess.Popen(cmd,stdout=o,stderr=e);(D/'running-exercise.json').write_text(json.dumps(dict(controller_pid=os.getpid(),pid=p.pid,command=cmd))+'\n')
 ackpath=root/'model/layout/r07-state.json'
 first=waitfor(lambda:(a if a and a['state']['replaced_bytes']>0 else None) if (a:=read(ackpath)) is not None else None,p)
 facts.append(dict(kind='first_actual_replacement',ack=first,time_monotonic_ns=time.monotonic_ns()))
 waitfor(lambda:any(x.get('snapshot_admitted',{}).get('generation')==2 for x in events()),p)
 snap_pending=root/'model/layout/snapshots/snapshot-000002.pending'
 bound=read(ackpath)
 assert bound['state']['replaced_bytes']>0,bound
 pause=root/'r07-provider-pause';pause.touch(exist_ok=False)
 facts.append(dict(kind='provider_publication_paused_during_second_snapshot',ack=bound,pending_exists=snap_pending.exists(),time_monotonic_ns=time.monotonic_ns()))
 try:
  revoked=waitfor(lambda:(a if a and a['state']['replaced_bytes']==0 and a['permitted_bytes']==0 else None) if (a:=read(ackpath)) is not None else None,p,10)
  assert revoked['owner']==bound['owner'] and revoked['state']['mapped_bytes']+revoked['state']['arena_tail_bytes']==revoked['state']['logical_bytes']
  facts.append(dict(kind='expired_reading_repaid_and_initialized',ack=revoked,time_monotonic_ns=time.monotonic_ns()))
  time.sleep(.8)
  lines=(root/'host-samples.jsonl').read_text().splitlines()
  paused=[json.loads(x) for x in lines if x.endswith('}') and json.loads(x).get('r07_provider_paused')]
  assert len(paused)>=3,len(paused)
  facts.append(dict(kind='sampler_continued_during_provider_pause',frames=len(paused),first=paused[0],last=paused[-1]))
 finally:
  pause.unlink(missing_ok=True)
 code=p.wait(timeout=120)
report=read(root/'observer-report.json');(D/'exercise-precheck.json').write_text(json.dumps(dict(exit=code,actual_seconds=time.monotonic()-start,facts=facts,observer_report=report),indent=2)+'\n')
assert code==0 and report['status']=='passed_measurement_wrapper',(code,report)
snapshot=root/'model/layout/snapshots/snapshot-000002';index=read(snapshot/'index.json');state=index['manifest']['r07'];assert index['schema']=='mycity-layout-snapshot-v3' and state['replaced_bytes']>0,index['manifest']
assert state['mapped_bytes']+state['replaced_bytes']+state['arena_tail_bytes']==state['logical_bytes']
assert index['manifest']['normal_joint_budget_bytes']==3328599654 and index['manifest']['cow_reserve_bytes']==128*1024**2
within=read(root/'model/cold-with-provider/report.json');assert within['cold_reopen_and_scan']=='passed'
# A genuinely new process with no provider must rebuild full R07 without old credit.
env={k:v for k,v in os.environ.items() if not k.startswith('MYCITY_')};cold=W/'independent-cold';cold_cmd=[exe,'layout-load-only',str(snapshot),str(cold)];t=time.monotonic()
with (D/'independent-cold.stdout').open('x') as o,(D/'independent-cold.stderr').open('x') as e:
 cold_process=subprocess.Popen(cold_cmd,env=env,stdout=o,stderr=e);cold_code=cold_process.wait(timeout=120)
assert cold_code==0
cold_report=read(cold/'report.json');fresh=cold_report['manifest']['r07'];assert cold_report['cold_reopen_and_scan']=='passed' and fresh['replaced_bytes']==0 and fresh['reading_sequence']==0
assert fresh['mapped_bytes']+fresh['arena_tail_bytes']==fresh['logical_bytes']==state['logical_bytes']
result=dict(source_sha256=info['source_sha256'],binary_sha256=info['binary_sha256'],exit=code,actual_seconds=time.monotonic()-start,facts=facts,second_snapshot_r07=state,independent_cold=dict(pid=cold_process.pid,exit=cold_code,seconds=time.monotonic()-t,manifest_r07=fresh,report_path=str(cold/'report.json')),observer_report=report,pool_report=read(root/'model/normal-pool/report.json'),original_record_counts_and_budgets_unchanged=True,formal_performance_accepted=False,observer_and_safety_never_paused=True,air_worker_compute_not_tested=True,real_16gb='not_run',eight_hour='not_run')
(D/'exercise-result.json').write_text(json.dumps(result,indent=2)+'\n');print(json.dumps(result,indent=2),flush=True)
