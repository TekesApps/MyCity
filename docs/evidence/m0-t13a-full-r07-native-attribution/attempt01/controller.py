"""Original frozen full-L1 R07 command plus measured, intrusive VM diagnostics.
No performance acceptance, pressure credit, workload/threshold change, or restart.
"""
from pathlib import Path
import hashlib,importlib.util,json,os,subprocess,time
repo=Path('/Users/frankqin/dev/MyCity');base=repo/'docs/evidence/m0-t13a-extents-joint-retry-02';work=Path('/Volumes/HP P900/dev/MyCity/work/full-r07-vm-diagnostic-01');assert not work.exists()
def load(name,path):
 spec=importlib.util.spec_from_file_location(name,path);m=importlib.util.module_from_spec(spec);spec.loader.exec_module(m);return m
metrics=load('existing_native_resource_reader',repo/'docs/evidence/m0-t13a-typed-workspace/observe.py')
def sha(p):
 h=hashlib.sha256()
 with Path(p).open('rb') as f:
  for b in iter(lambda:f.read(1048576),b''):h.update(b)
 return h.hexdigest()
info=json.loads((base/'source.json').read_text());binary=Path(info['binary']);assert sha(binary)==info['binary_sha256'];assert sha(repo/'fixtures/m0-t13a-acceptance.json')==info['acceptance_sha256'];assert sha(repo/'Cargo.lock')==info['lock_sha256']
# Refuse the initial launch if a builder or another model is currently active.
proc=subprocess.check_output(['ps','-axo','pid=,comm='],text=True)
for line in proc.splitlines():
 fields=line.strip().split(None,1)
 if len(fields)!=2:continue
 name=Path(fields[1]).name
 assert name not in ['cargo','rustc','cargo-clippy','clippy-driver','mycity-headless'],line
assert metrics.pressure()==1
s=os.statvfs(work.parent);assert s.f_bavail*s.f_frsize>20*1024**3
work.mkdir();(work/'captures').mkdir();observed=work/'observed';argv=[str(binary),'native-observe',str(observed),'--',str(binary),'r07-layout-driver',str(observed/'model'),'--vm-trace']
intent={'argv':argv,'binary_sha256':info['binary_sha256'],'source_sha256':info['source_sha256'],'frozen_source_zip_sha256':sha(base/'frozen-source.zip'),'acceptance_sha256':info['acceptance_sha256'],'scope':'full original L1/R07 diagnostic, external vmmap and controller perturbation; NOT acceptance','storage':'HP P900 external USB/APFS SSD','normal_pool_bytes':67108864,'full_r07_mapped_bytes':277610496,'workload_or_threshold_changed':False,'compile_or_new_target':False,'auxiliary_accounting':'controller and vmmap helpers sampled separately; never counted as zero or deducted from physical group','retention':'retain complete outputs until recovery/dependency review; no original/failure snapshot deletion','processes_before_comm_only':proc,'filesystem_available_before_bytes':s.f_bavail*s.f_frsize,'controller_script_sha256':sha(__file__),'native_reader_script_sha256':sha(repo/'docs/evidence/m0-t13a-typed-workspace/observe.py')}
(work/'intent.json').write_text(json.dumps(intent,indent=2));env=os.environ.copy()
for key in list(env):
 if key.startswith('MYCITY_') or key=='RUST_MIN_STACK':env.pop(key)
out=(work/'observer.stdout').open('xb');err=(work/'observer.stderr').open('xb');p=subprocess.Popen(argv,env=env,stdout=out,stderr=err);start=time.monotonic_ns();(work/'live.json').write_text(json.dumps({'controller_pid':os.getpid(),'observer_pid':p.pid,'begin_monotonic_ns':start},indent=2))
last=None;offset=0;buffer=b'';trigger=None;children=[];reduced_seen=False;peak_aux=0;capture_complete=False
with (work/'auxiliary-samples.jsonl').open('x') as aux:
 while p.poll() is None or any(c['process'].poll() is None for c in children):
  frames=observed/'host-samples.jsonl'
  if frames.exists():
   with frames.open('rb') as f:f.seek(offset);data=f.read();offset+=len(data)
   buffer+=data;lines=buffer.split(b'\n');buffer=lines.pop()
   for line in lines:
    if not line:continue
    frame=json.loads(line);state=((frame.get('r07_applied') or {}).get('state') or {})
    if state.get('mapped_bytes',277610496)<277610496:reduced_seen=True
    last=frame
    if trigger is None and reduced_seen and frame.get('group_capture_complete') and state.get('mapped_bytes')==277610496 and state.get('epoch',0)>=3:
     roles={row[4]:row for row in frame['members']}
     required=['observer','r07-layout-driver','normal-pool-host','layout-r07-session']
     if not all(role in roles for role in required):continue
     trigger={'monotonic_ns':time.monotonic_ns(),'frame':frame,'reduced_seen':True};(work/'trigger.json').write_text(json.dumps(trigger,indent=2))
     for role in required:
      pid,parent,birth,rss,_=roles[role];prefix=work/'captures'/role;a=metrics.usage(pid)
      o=prefix.with_suffix('.stdout').open('xb');e=prefix.with_suffix('.stderr').open('xb');args=['/usr/bin/vmmap','-wide','-pages','-interleaved',str(pid)];child=subprocess.Popen(args,stdout=o,stderr=e)
      children.append({'role':role,'target_pid':pid,'expected_birth':birth,'before':a,'process':child,'out':o,'err':e,'argv':args,'begin_ns':time.monotonic_ns(),'helper_first':metrics.usage(child.pid),'saved':False})
     print('full-R07 capture triggered',flush=True)
  samples=[]
  controller=metrics.usage(os.getpid());missing=controller is None
  for c in children:
   child=c['process'];code=child.poll()
   if code is None:
    sample=metrics.usage(child.pid);samples.append({'role':c['role'],'helper_pid':child.pid,'usage':sample});missing|=sample is None
   elif not c['saved']:
    c['out'].close();c['err'].close();after=metrics.usage(c['target_pid']);before=c['before'];record={k:v for k,v in c.items() if k not in ['process','out','err','saved']};record.update(exit_code=code,end_ns=time.monotonic_ns(),after=after,helper_pid=child.pid,target_identity_unchanged=bool(before and after and before['start_abstime']==c['expected_birth']==after['start_abstime']),helper_missing_after_exit='not sampled as zero')
    (work/'captures'/(c['role']+'.json')).write_text(json.dumps(record,indent=2));c['saved']=True
  total=None if missing else controller['phys_footprint_bytes']+sum(x['usage']['phys_footprint_bytes'] for x in samples)
  if total is not None:peak_aux=max(peak_aux,total)
  aux.write(json.dumps({'monotonic_ns':time.monotonic_ns(),'controller':controller,'helpers':samples,'missing':missing,'observed_auxiliary_footprint_sum_bytes':total,'last_host_t':None if last is None else last.get('t')},separators=(',',':'))+'\n');aux.flush()
  time.sleep(.02)
code=p.wait();out.close();err.close()
summary={'observer_exit':code,'elapsed_observation_seconds':(time.monotonic_ns()-start)/1e9,'capture_triggered':trigger is not None,'capture_count':len(children),'all_capture_receipts_saved':all(c['saved'] for c in children),'peak_observed_auxiliary_footprint_bytes':peak_aux,'auxiliary_missing':'per-sample explicit; exited helpers not zero-cost proof','binary_still_same':sha(binary)==info['binary_sha256'],'memory_16gb_gate':'not_run','full_M0':'not_accepted','scope':'intrusive physical attribution only; not normal, speed or endurance acceptance'}
(work/'terminal.json').write_text(json.dumps(summary,indent=2));print(json.dumps(summary,indent=2),flush=True)
