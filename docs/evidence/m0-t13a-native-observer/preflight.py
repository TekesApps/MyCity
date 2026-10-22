"""Actual functional fault/lifecycle checks. Not model scale or performance acceptance."""
import fcntl,hashlib,json,os,signal,subprocess,time
from pathlib import Path
D=Path(__file__).resolve().parent;R=D.parents[2];os.chdir(R)
W=R/'work/native-watch-preflight-01';assert not W.exists();W.mkdir()
exe=str(R/'target/release/mycity-headless');hash_before=hashlib.sha256(Path(exe).read_bytes()).hexdigest()
results=[]
def frames(root):
 p=root/'host-samples.jsonl'
 if not p.exists():return []
 return [json.loads(x) for x in p.read_text().splitlines() if x.endswith('}')]
def waitfor(fn,seconds=10):
 stop=time.monotonic()+seconds
 while time.monotonic()<stop:
  v=fn()
  if v:return v
  time.sleep(.01)
 raise RuntimeError('functional timeout, no automatic restart')
def absent(pid):
 try:os.kill(pid,0);return False
 except ProcessLookupError:return True

def start(name,mode):
 root=W/name
 out=(D/(name+'.stdout')).open('x');err=(D/(name+'.stderr')).open('x')
 p=subprocess.Popen([exe,'native-observe',str(root),'--',exe,'native-watch-fixture',str(root/'model'),mode],stdout=out,stderr=err)
 return p,root,out,err

def done(p,root,out,err):
 code=p.wait(timeout=15);out.close();err.close()
 report=json.loads((root/'observer-report.json').read_text())
 return code,report
for mode in ['ok','delayed-owner','fail','no-clock','short-child','orphan']:
 p,root,o,e=start(mode,mode);code,report=done(p,root,o,e)
 expected=0 if mode in ['ok','delayed-owner','short-child'] else 2
 # Orphan may finish during the post-root grace frames; actual membership must
 # still include the journal identity and cleanup cannot claim never existed.
 if mode=='orphan':assert code in [0,2]
 else:assert code==expected,(mode,code,report)
 assert report['cleanup_error'] is None,(mode,report)
 assert report['final_live_identity_count']==1,(mode,report)
 f=frames(root)
 if mode=='short-child':assert report['unpolled_registered_processes']>=1
 if mode=='orphan':assert report['identities_recorded']>=3
 results.append(dict(mode=mode,exit=code,report=report,frames=len(f),scope='functional only'))
 print(mode,code,report['samples'],flush=True)
# Observer loss while the root is delayed before initial parent verification.
p,root,o,e=start('kill-before-owner','delayed-owner')
child=waitfor(lambda:json.loads((root/'observer-child.json').read_text()) if (root/'observer-child.json').exists() else None)['identity']['pid']
p.kill();assert p.wait(timeout=5)==-signal.SIGKILL
waitfor(lambda:absent(child));o.close();e.close()
assert not (root/'model/fixture-result.json').exists()
assert 'native_owner_parent_changed' in (root/'model.stderr').read_text()
results.append(dict(mode='kill-before-owner',observer_exit=-signal.SIGKILL,root_pid=child,root_absent=True,business_result_absent=True,scope='no final observer report expected after observer SIGKILL'))
# Active owner EOF must terminate the full private root+leaf group, not the
# unrelated process group owned by this external harness.
outsider=subprocess.Popen(['/bin/sleep','30'],start_new_session=True)
try:
 p,root,o,e=start('kill-owner-tree','hang-tree')
 leaf=waitfor(lambda:json.loads((root/'model/leaf-pid.json').read_text()) if (root/'model/leaf-pid.json').exists() else None)
 child=json.loads((root/'observer-child.json').read_text())['identity']['pid']
 waitfor(lambda:len(frames(root))>=2)
 assert os.getpgid(child)==child and os.getpgid(leaf)==child
 p.kill();assert p.wait(timeout=5)==-signal.SIGKILL
 waitfor(lambda:absent(child) and absent(leaf));assert outsider.poll() is None
 o.close();e.close();results.append(dict(mode='kill-owner-tree',root_pid=child,leaf_pid=leaf,both_absent=True,outsider_pid=outsider.pid,outsider_unchanged=True,observer_exit=-signal.SIGKILL))
finally:
 outsider.terminate();outsider.wait(timeout=5)
# Keep the journal exclusively locked while sampling continues. Busy must make
# every affected group frame incomplete; no publisher or read lock may hang.
p,root,o,e=start('locked-journal','hang')
waitfor(lambda:len(frames(root))>=2)
with (root/'registrations.jsonl').open('rb') as lock:
 fcntl.flock(lock,fcntl.LOCK_EX);start_index=len(frames(root));time.sleep(1.3)
 held=frames(root)[start_index:];assert len(held)>=3 and all(x['registration_lock_busy'] and not x['group_capture_complete'] for x in held),held
 assert all(x['pressure_dispatch_flag'] in [1,2,4] and 'swap_used_bytes' in x for x in held)
 fcntl.flock(lock,fcntl.LOCK_UN)
child=json.loads((root/'observer-child.json').read_text())['identity']['pid'];os.kill(child,signal.SIGTERM)
code,report=done(p,root,o,e);assert code==2 and report['cleanup_error'] is None
results.append(dict(mode='locked-journal',exit=code,busy_frames=len(held),sampling_and_pressure_continued=True,report=report))
assert hashlib.sha256(Path(exe).read_bytes()).hexdigest()==hash_before
(D/'preflight-result.json').write_text(json.dumps(dict(binary_sha256=hash_before,results=results,formal_acceptance=False,no_new_target=True),indent=2)+'\n')
print('all lifecycle cases completed',flush=True)
