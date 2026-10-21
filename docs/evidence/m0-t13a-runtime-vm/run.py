"""One original full L1 smoke, VM diagnosis only; never restarts or accepts performance."""
import concurrent.futures,ctypes,hashlib,importlib.util,json,os,struct,subprocess,sys,time,zipfile
from pathlib import Path
D=Path(__file__).resolve().parent;R=D.parents[2];P=D.parent/'m0-t13a-typed-workspace';os.chdir(R)
sys.path.insert(0,str(P));import native_metrics as native
spec=importlib.util.spec_from_file_location('observer_metrics',P/'observe.py');metrics=importlib.util.module_from_spec(spec);spec.loader.exec_module(metrics)
W=R/'work/runtime-vm-l1-01';assert not W.exists()
sha=lambda p:hashlib.sha256(Path(p).read_bytes()).hexdigest()
info=json.loads((P/'source.json').read_text());freeze=json.loads((P/'freeze.json').read_text())
h=hashlib.sha256()
with zipfile.ZipFile(P/'frozen-source.zip') as z:
 for name in sorted(z.namelist()):
  b=z.read(name);assert (R/name).read_bytes()==b,name;n=name.encode();h.update(struct.pack('<Q',len(n)));h.update(n);h.update(struct.pack('<Q',len(b)));h.update(b)
assert h.hexdigest()==info['source_sha256'];assert sha(info['binary'])==info['binary_sha256']
assert sha(R/'fixtures/m0-t13a-acceptance.json')==freeze['acceptance_sha256'];assert sha(R/'Cargo.lock')==freeze['lock_sha256']
assert metrics.pressure()==1
peak=3*3378784402+384*1024**2;available=os.statvfs(R).f_bavail*os.statvfs(R).f_frsize;assert available>peak+2*1024**3
W.mkdir()
intent=dict(schema='mycity-vm-diagnostic-intent-v1',source_sha256=info['source_sha256'],binary_sha256=info['binary_sha256'],scope='Original L1 full records, 15s existing smoke, NOT formal performance acceptance. VM tools perturb this run.',estimated_peak_bytes=peak,filesystem_available_before_bytes=available,no_new_target=True,retention='No new recovery baseline. Terminal synthetic pressure/reserve and duplicate components eligible for exact retirement after report/readback/dependency audit; keep raw VM, raw measurement, index, logs, source/exe; protected prior current7/8 untouched.',protected_existing=['work/typed-workspace-formal-l1-01/observed/model/layout/snapshots/snapshot-000007','work/typed-workspace-formal-l1-01/observed/model/layout/snapshots/snapshot-000008'],tool='vmmap -wide -pages -interleaved; no forkCorpse',threshold_or_workload_change=False)
(W/'retention-intent.json').write_text(json.dumps(intent,indent=2)+'\n');(D/'intent.json').write_text(json.dumps(intent,indent=2)+'\n')
for key in ('MYCITY_NORMAL_POOL','MYCITY_BROKER_SESSION','MYCITY_BROKER_SUPERVISOR','MYCITY_NORMAL_WORKER_GRANT','MYCITY_ROLE_CONSTRUCTION_OOM'):os.environ.pop(key,None)
cmd=[sys.executable,str(P/'observe.py'),str(W/'observed'),'--',info['binary'],'steady-run',str(W/'observed/model'),'l1','smoke']
(D/'live.json').write_text(json.dumps(dict(pid=os.getpid(),command=cmd,start_monotonic_ns=time.monotonic_ns()),indent=2)+'\n')
def tail_json(path):
 try:
  with path.open('rb') as f:
   f.seek(0,2);f.seek(max(0,f.tell()-131072));rows=f.read().splitlines()
  return [json.loads(x) for x in rows if x.startswith(b'{') and x.endswith(b'}')]
 except FileNotFoundError:return []
def phase():
 rows=tail_json(W/'observed/model/layout/events.jsonl');scans=[x for x in rows if 'scan' in x]
 return scans[-1] if scans else None

def identity(pid):
 start=time.monotonic_ns();a=metrics.usage(pid);rel=native.relation(pid);b=metrics.usage(pid)
 ok=bool(a and b and rel and a['start_abstime']==b['start_abstime'])
 return dict(pid=pid,begin_monotonic_ns=start,end_monotonic_ns=time.monotonic_ns(),first=a,last=b,relation=rel,identity_bound=ok)

def capture(label,role,pid):
 before=identity(pid);event_before=phase();start=time.monotonic_ns();prefix=D/f'{label}-{role}-{pid}'
 try:
  with prefix.with_suffix('.stdout').open('x') as out,prefix.with_suffix('.stderr').open('x') as err:
   result=subprocess.run(['/usr/bin/vmmap','-wide','-pages','-interleaved',str(pid)],stdout=out,stderr=err,timeout=20);code=result.returncode;error=None
 except subprocess.TimeoutExpired:code=None;error='vmmap timeout; partial output retained, no target signal'
 end=time.monotonic_ns();after=identity(pid);event_after=phase()
 same=bool(before['identity_bound'] and after['identity_bound'] and before['last']['start_abstime']==after['last']['start_abstime'] and before['relation']==after['relation'])
 record=dict(label=label,role=role,pid=pid,before=before,after=after,tool_begin_monotonic_ns=start,tool_end_monotonic_ns=end,exit_code=code,error=error,identity_unchanged=same,event_before=event_before,event_after=event_after,stdout=str(prefix.with_suffix('.stdout').relative_to(R)),stderr=str(prefix.with_suffix('.stderr').relative_to(R)),ownership='raw mapping evidence only; no pressure replacement granted')
 prefix.with_suffix('.json').write_text(json.dumps(record,indent=2)+'\n');return record
captures=[];labels=set();begin=time.monotonic()
with (D/'observer.stdout').open('x') as out,(D/'observer.stderr').open('x') as err:
 child=subprocess.Popen(cmd,stdout=out,stderr=err)
 (D/'observer-pid.json').write_text(json.dumps(dict(pid=child.pid,identity=identity(child.pid)),indent=2)+'\n')
 try:
  while child.poll() is None:
   event=phase();frames=tail_json(W/'observed/host-samples.jsonl')
   if event and frames:
    frame=frames[-1];members=frame.get('members',[]);rows=native.process_table();roster={}
    for pid,parent,birth,rss,role in members:
     if pid==child.pid:roster['observer']=pid
     elif parent==child.pid:roster['root']=pid
     else:
      # Proc args distinguish same-executable roles; retain command as evidence.
      p=subprocess.run(['/bin/ps','-p',str(pid),'-o','command='],capture_output=True,text=True)
      args=p.stdout
      if 'normal-pool-host' in args:roster['pool']=pid
      elif 'steady-layout-session' in args:roster['layout']=pid
      elif 'steady-driver' in args:roster['driver']=pid
    label='inflight' if event['snapshot_inflight'] else 'idle'
    if len(roster)==5 and label not in labels:
     labels.add(label);(D/f'{label}-trigger.json').write_text(json.dumps(dict(monotonic_ns=time.monotonic_ns(),roster=roster,frame=frame,event=event),indent=2)+'\n')
     with concurrent.futures.ThreadPoolExecutor(max_workers=5) as pool:captures.extend(pool.map(lambda pair:capture(label,*pair),roster.items()))
   if time.monotonic()-begin>180:raise RuntimeError('diagnostic deadline; no automatic restart')
   time.sleep(.05)
  code=child.wait()
 finally:
  if child.poll() is None:
   # Do not kill a live original run merely because diagnostic capture failed.
   (D/'capture-error-live.json').write_text(json.dumps(dict(observer_pid=child.pid,still_live=True))+'\n')
(D/'terminal.json').write_text(json.dumps(dict(observer_exit=code,actual_seconds=time.monotonic()-begin,labels=sorted(labels),captures=len(captures),all_identity_unchanged=all(x['identity_unchanged'] for x in captures),all_vmmap_exit_zero=all(x['exit_code']==0 for x in captures),formal_acceptance='not_accepted',threshold_or_workload_change=False,binary_unchanged=sha(info['binary'])==info['binary_sha256']),indent=2)+'\n')
print((D/'terminal.json').read_text())
