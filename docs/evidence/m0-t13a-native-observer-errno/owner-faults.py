"""Fault-only original L1 starts: kill observer at real init/compute/write boundaries.
Never formal performance attempts; no automatic restart, no smaller workload.
"""
import ctypes,hashlib,importlib.util,json,os,signal,subprocess,sys,time
from pathlib import Path
D=Path(__file__).resolve().parent;R=D.parents[2];P=D.parent/'m0-t13a-typed-workspace';os.chdir(R);sys.path.insert(0,str(P));import native_metrics as native
spec=importlib.util.spec_from_file_location('fault_sdk',P/'observe.py');old=importlib.util.module_from_spec(spec);spec.loader.exec_module(old)
info=json.loads((D/'source.json').read_text());exe=info['binary'];assert hashlib.sha256(Path(exe).read_bytes()).hexdigest()==info['binary_sha256']
W=R/'work/native-observer-faults-01';assert W.exists()  # resume only new cases; original init remains terminal
(W/'intent.json').write_text(json.dumps(dict(purpose='owner-loss termination at actual L1 init/worker/write; not crash recovery or performance test',estimated_peak_bytes=12*1024**3,terminal='preserve raw/group birth and partial file evidence; synthetic pressure bytes eligible for exact retirement only after advisor release, not a required subsequent restore input',original_record_counts=True,no_new_target=True),indent=2)+'\n')
assert os.statvfs(R).f_bavail*os.statvfs(R).f_frsize>14*1024**3
MODE=native.constant('sys/proc_info.h','PROC_PGRP_ONLY')
def readrows(p):
 if not p.exists():return []
 return [json.loads(x) for x in p.read_text().splitlines() if x.endswith('}')]
def usage(pid):
 v=old.Usage();ret=old.lib.proc_pid_rusage(pid,4,ctypes.byref(v));return None if ret else dict(pid=pid,birth=v.ri_proc_start_abstime,footprint_bytes=v.ri_phys_footprint,exit_abstime=v.ri_proc_exit_abstime)
def members(group):
 a=(ctypes.c_uint32*8192)();n=native.lib.proc_listpids(MODE,group,a,ctypes.sizeof(a));assert 0<=n<ctypes.sizeof(a) and n%4==0
 return [u for pid in a[:n//4] if pid and (u:=usage(pid))]
def absent(pid):
 try:os.kill(pid,0);return False
 except ProcessLookupError:return True
outsider=subprocess.Popen(['/bin/sleep','120'],start_new_session=True);results=json.loads((D/'owner-fault-results.json').read_text())
try:
 for mode in ['worker-compute-l2','snapshot-write']:
  root=W/mode;cmd=[exe,'native-observe',str(root),'--',exe,'steady-run',str(root/'model'),('l2' if mode=='worker-compute-l2' else 'l1'),'formal'];begin=time.monotonic();trigger=None
  with (D/(mode+'.stdout')).open('x') as o,(D/(mode+'.stderr')).open('x') as e:
   p=subprocess.Popen(cmd,stdout=o,stderr=e)
   (D/'fault-running.json').write_text(json.dumps(dict(controller_pid=os.getpid(),pid=p.pid,mode=mode,command=cmd))+'\n')
   while p.poll() is None:
    rows=readrows(root/'registrations.jsonl');layout=[x for x in rows if x['kind']=='started' and x['role']=='steady-layout-session'];tasks=[x for x in rows if x['kind']=='started' and x['role'] in ['scale-model-task','scale-sim-probe']]
    if mode=='initializing' and layout and not (root/'model/layout/manifest.json').exists():trigger=dict(kind='registered layout before its full ready manifest',registration=layout[-1])
    if mode=='worker-compute-l2' and tasks:
     task=tasks[-1];u=usage(task['sample']['identity']['pid'])
     if u and u['birth']==task['sample']['identity']['start_abstime']:trigger=dict(kind='actual model task registered and alive',registration=task,actual_usage=u)
    if mode=='snapshot-write':
     candidates=list((root/'model/layout/snapshots').glob('*.pending/pressure.bin')) if (root/'model/layout/snapshots').exists() else []
     if candidates:
      f=candidates[0];size=f.stat().st_size
      if 0<size<3044040310:trigger=dict(kind='actual original snapshot pressure file partially written',path=str(f.relative_to(R)),bytes=size,expected_full_bytes=3044040310)
    if trigger:
     identity=json.loads((root/'observer-child.json').read_text())['identity'];group=identity['pid'];assert os.getpgid(group)==group
     roster=members(group);assert len(roster)>=2 and all(os.getpgid(x['pid'])==group for x in roster)
     trigger_time=time.monotonic_ns();p.kill();code=p.wait(timeout=5);assert code==-signal.SIGKILL
     deadline=time.monotonic()+10
     while time.monotonic()<deadline and any(not absent(x['pid']) for x in roster):time.sleep(.01)
     assert all(absent(x['pid']) for x in roster),roster
     assert outsider.poll() is None and absent(group)
     files=[]
     for f in root.rglob('*'):
      if f.is_file():s=f.stat();files.append(dict(path=str(f.relative_to(R)),bytes=s.st_size,allocated_bytes=s.st_blocks*512))
     record=dict(mode=mode,command=cmd,trigger=trigger,trigger_monotonic_ns=trigger_time,observer_exit=code,group=group,members_before=roster,all_group_members_absent=True,outsider_alive=True,elapsed_seconds=time.monotonic()-begin,observer_final_report_exists=(root/'observer-report.json').exists(),files_after=files,scope='intentional abort; missing footer is failure, not normal return; persistence/crash recovery not validated')
     results.append(record);(D/'owner-fault-results.json').write_text(json.dumps(results,indent=2)+'\n');print(mode,len(roster),record['elapsed_seconds'],flush=True);break
    assert time.monotonic()-begin<90,'trigger not reached; preserve running process and inspect; no restart'
    time.sleep(.002)
   if trigger is None:raise RuntimeError('root ended before expected fault boundary; inspect existing run, do not restart')
finally:
 outsider.terminate();outsider.wait(timeout=5)
print('actual init(L1), compute(L2), write(L1) owner-loss cases completed; initial impossible L1 compute trigger separately retained',flush=True)
