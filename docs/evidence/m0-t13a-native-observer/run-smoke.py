"""Frozen original-full L1 smoke plus independent SDK observations; never formal acceptance."""
import ctypes,hashlib,importlib.util,json,os,struct,subprocess,sys,time,zipfile
from pathlib import Path
D=Path(__file__).resolve().parent;R=D.parents[2];os.chdir(R);P=D.parent/'m0-t13a-typed-workspace'
sys.path.insert(0,str(P));import native_metrics as native
spec=importlib.util.spec_from_file_location('sdk_oracle',P/'observe.py');old=importlib.util.module_from_spec(spec);spec.loader.exec_module(old)
info=json.loads((D/'source.json').read_text());binary=info['binary'];freeze=json.loads((D/'freeze.json').read_text())
sha=lambda p:hashlib.sha256(Path(p).read_bytes()).hexdigest()
assert sha(binary)==info['binary_sha256'];assert sha('fixtures/m0-t13a-acceptance.json')==freeze['acceptance_sha256']
with zipfile.ZipFile(D/'frozen-source.zip') as z:
 for name in z.namelist():assert (R/name).read_bytes()==z.read(name),name
W=R/'work/native-observer-l1-01';assert not W.exists();W.mkdir();root=W/'observed'
available=os.statvfs(R).f_bavail*os.statvfs(R).f_frsize;peak=3*3378784402+128*1024**2;assert available>peak+2*1024**3
intent=dict(source_sha256=info['source_sha256'],original_load=True,mode='3s+12s existing smoke, not600/1800 or8h',estimated_new_peak_bytes=peak,available_before_bytes=available,retention='terminal payload eligible after actual cold restore and advisor release; keep SDK raw, samples, journal, reports, original indexes/components, source/lock/exe; existing protected originals unchanged',external_oracle='Python SDK harness excluded from native group; diagnostic perturbation, cannot accept physical performance')
(W/'intent.json').write_text(json.dumps(intent,indent=2)+'\n')
for key in ['MYCITY_NORMAL_POOL','MYCITY_BROKER_SESSION','MYCITY_BROKER_SUPERVISOR','MYCITY_NORMAL_WORKER_GRANT','MYCITY_ROLE_CONSTRUCTION_OOM']:os.environ.pop(key,None)
cmd=[binary,'native-observe',str(root),'--',binary,'steady-run',str(root/'model'),'l1','smoke'];began=time.monotonic();count=0;seen=set();probes=[]
with (D/'smoke.stdout').open('x') as out,(D/'smoke.stderr').open('x') as err,(D/'sdk-crosscheck.jsonl').open('x') as raw:
 p=subprocess.Popen(cmd,stdout=out,stderr=err);(D/'smoke-running.json').write_text(json.dumps(dict(controller_pid=os.getpid(),pid=p.pid,command=cmd))+'\n')
 while p.poll() is None:
  f=root/'host-samples.jsonl'
  if f.exists():
   lines=f.read_text().splitlines();valid=[x for x in lines if x.endswith('}')]
   if len(valid)>count:
    frame=json.loads(valid[-1]);count=len(valid);swap=old.sysctl('vm.swapusage',old.Swap());pressure=old.sysctl('kern.memorystatus_vm_pressure_level',ctypes.c_int())
    oracle=[]
    for pid,parent,birth,rss,role in frame['members']:
     first=old.Usage();last=old.Usage();begin=time.monotonic_ns();r1=old.lib.proc_pid_rusage(pid,4,ctypes.byref(first));rel=native.relation(pid);r2=old.lib.proc_pid_rusage(pid,4,ctypes.byref(last));end=time.monotonic_ns()
     matched=r1==r2==0 and rel is not None and first.ri_proc_start_abstime==last.ri_proc_start_abstime==birth
     if matched:seen.add((pid,birth))
     oracle.append(dict(pid=pid,role=role,expected_birth=birth,begin_monotonic_ns=begin,end_monotonic_ns=end,rusage_codes=[r1,r2],relation=rel,birth_matched=matched,usage=None if r1 or r2 else dict(birth=last.ri_proc_start_abstime,rss_bytes=last.ri_resident_size,footprint_bytes=last.ri_phys_footprint,user_time_raw=last.ri_user_time,system_time_raw=last.ri_system_time,disk_read_bytes=last.ri_diskio_bytesread,disk_written_bytes=last.ri_diskio_byteswritten)))
    row=dict(native_frame_index=count-1,frame=frame,oracle=oracle,swap_sdk=dict(size=ctypes.sizeof(swap),total=swap.total,available=swap.available,used=swap.used,page_size=swap.page_size,encrypted=swap.encrypted),pressure_sdk=pressure.value,scope='sequential independent readings; volatile footprint/RSS not expected bit-identical, external harness not included in performance group')
    raw.write(json.dumps(row)+'\n');raw.flush();probes.append(row)
  assert time.monotonic()-began<180,'smoke exceeded functional deadline; do not restart while live'
  time.sleep(.05)
 code=p.wait()
report=json.loads((root/'observer-report.json').read_text());assert code==0 and report['status']=='passed_measurement_wrapper',(code,report)
assert report['arena']['activation']['grant_bytes']==2*1024**2 and report['arena']['allocation_failures']==0
assert report['combined_diagnostic_hot_bound_bytes']<=8*1024**2
assert report['memory_credit_bytes']==0 and sha(binary)==info['binary_sha256']
assert len(seen)>=5 and probes
assert all(x['swap_sdk']['size']==32 and x['pressure_sdk'] in [1,2,4] for x in probes)
assert all(x['frame']['pressure_dispatch_flag']==x['pressure_sdk'] for x in probes),'pressure changed across SDK reads; investigate raw without overriding'
result=dict(command=cmd,exit=code,actual_seconds=time.monotonic()-began,source_sha256=info['source_sha256'],binary_sha256=info['binary_sha256'],unique_pid_births_independently_matched=len(seen),sdk_probe_frames=len(probes),observer_report=report,original_load_thresholds_unchanged=True,formal_acceptance=False,memory_16gb_gate='not_run',eight_hour_gate='not_run',memory_credit_bytes=0)
(D/'smoke-result.json').write_text(json.dumps(result,indent=2)+'\n');print(json.dumps(result,indent=2),flush=True)
