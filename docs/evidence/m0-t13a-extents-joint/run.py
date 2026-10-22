"""One original full-L1 experiment, actual address lists and VM brackets.
Read-only external VM tools perturb this diagnostic. Never a formal performance run.
"""
import concurrent.futures,hashlib,importlib.util,json,os,subprocess,sys,time,zipfile
from pathlib import Path
D=Path(__file__).resolve().parent;R=D.parents[2];P=D.parent/'m0-t13a-typed-workspace';os.chdir(R)
sys.path.insert(0,str(D));from topology import assert_pinned_identity,assert_repaid_identity,r07_extents
sys.path.insert(0,str(P));import native_metrics as native
spec=importlib.util.spec_from_file_location('vmsdk',P/'observe.py');sdk=importlib.util.module_from_spec(spec);spec.loader.exec_module(sdk)
info=json.loads((D/'source.json').read_text());exe=info['binary'];sha=lambda p:hashlib.sha256(Path(p).read_bytes()).hexdigest();assert sha(exe)==info['binary_sha256']
with zipfile.ZipFile(D/'frozen-source.zip')as z:
 for n in z.namelist():assert (R/n).read_bytes()==z.read(n),n
W=R/'work/m0-t13a-extents-joint-l1-01';assert not W.exists();assert os.statvfs(R).f_bavail*os.statvfs(R).f_frsize>10*1024**3;W.mkdir();root=W/'observed';layout=root/'model/layout';ackpath=layout/'r07-state.json'
(W/'intent.json').write_text((D/'retention-intent.json').read_text())
def save(n,v): (D/n).write_text(json.dumps(v,indent=2)+'\n')
def read(p):
 try:return json.loads(p.read_text())
 except FileNotFoundError:return None

def events():
 p=layout/'events.jsonl'
 if not p.exists():return []
 return [json.loads(x)for x in p.read_text().splitlines()if x.endswith('}')]
def topology(p,expected_mapped=None):
 assert p.suffix=='.jsonl' and p.stat().st_size<=2*1024**2
 rows=[json.loads(x)for x in p.read_text().splitlines()];h=rows[0];f=rows[-1];body=rows[1:-1]
 assert h['record']=='begin' and f['record']=='end' and f['complete'] and h['owner']==f['owner'] and h['epoch']==f['epoch'] and h['list_id']==f['list_id']
 assert h['source_sha256']==info['source_sha256'] and h['expected_ranges']==f['actual_ranges']==len(body)
 assert all(0<x['len']<=x['capacity'] and x['start']>0 for x in body)
 r07=[x for x in body if x.get('kind')=='r07_immutable_map']
 extents=r07_extents(dict(ranges=body),expected_mapped)if r07 else []
 return dict(path=str(p),sha256=sha(p),header=h,footer=f,ranges=body,r07_extents=extents,r07_mapped_bytes=sum(x['logical_len']for x in extents))
def context():
 ev=events();scans=[x for x in ev if 'scan'in x];lists=[x['vm_regions']for x in ev if 'vm_regions'in x]
 if not scans or not lists:return None
 a=read(ackpath)
 if a is None:return None
 t=topology(Path(lists[-1]['file']),a['state']['mapped_bytes']);adm=[x['snapshot_admitted']['generation']for x in ev if 'snapshot_admitted'in x];done=[x['snapshot_durable']['generation']for x in ev if 'snapshot_durable'in x]
 return dict(scan=scans[-1],topology=dict(path=t['path'],sha256=t['sha256'],header=t['header'],footer=t['footer'],r07_extents=t['r07_extents'],r07_mapped_bytes=t['r07_mapped_bytes']),ack=a,admitted=max(adm,default=0),durable=max(done,default=0),phase='inflight'if max(adm,default=0)>max(done,default=0)else'idle')
def waitfor(fn,p,limit=90):
 end=time.monotonic()+limit
 while time.monotonic()<end:
  v=fn()
  if v:return v
  if p.poll()is not None:raise RuntimeError('existing wrapper terminal; inspect, never restart')
  time.sleep(.01)
 raise RuntimeError('observation timeout; existing handle must be inspected, not restarted')
def identity(pid):
 a=sdk.usage(pid);rel=native.relation(pid);b=sdk.usage(pid)
 return dict(first=a,last=b,relation=rel,valid=bool(a and b and rel and a['start_abstime']==b['start_abstime']))
def capture(label,role,top):
 pid=top['header']['owner']['pid'];before=identity(pid);cb=context();beg=time.monotonic_ns();prefix=D/f'{label}-{role}-{pid}'
 try:
  with prefix.with_suffix('.stdout').open('x')as o,prefix.with_suffix('.stderr').open('x')as e:
   q=subprocess.run(['/usr/bin/vmmap','-wide','-pages','-interleaved',str(pid)],stdout=o,stderr=e,timeout=20);code=q.returncode;error=None
 except subprocess.TimeoutExpired:code=None;error='vmmap_timeout_partial_retained'
 end=time.monotonic_ns();after=identity(pid);ca=context();owner=top['header']['owner']
 same=bool(before['valid'] and after['valid'] and before['last']['start_abstime']==after['last']['start_abstime']==owner['start_abstime'] and before['relation']==after['relation'])
 stable=bool(cb and ca and cb['phase']==ca['phase']==label and cb['ack']['state']['epoch']==ca['ack']['state']['epoch']==cb['topology']['header']['epoch']==ca['topology']['header']['epoch'] and cb['topology']['sha256']==ca['topology']['sha256'])
 if label=='inflight'and role=='layout':assert_pinned_identity(pinned_baseline,ca['topology']['r07_extents'])
 x=dict(label=label,role=role,pid=pid,identity_before=before,identity_after=after,identity_unchanged=same,context_before=cb,context_after=ca,topology=top,tool_begin_monotonic_ns=beg,tool_end_monotonic_ns=end,exit_code=code,error=error,stable_phase_epoch_list_endpoints=stable,stdout=str(prefix.with_suffix('.stdout').relative_to(R)),stderr=str(prefix.with_suffix('.stderr').relative_to(R)),scope='asynchronous perturbed address attribution only, no physical credit')
 save(prefix.name+'.json',x);return x
cmd=[exe,'native-observe',str(root),'--',exe,'r07-layout-driver',str(root/'model'),'--vm-trace'];start=time.monotonic();facts=[];captures=[];pinned_baseline=[]
with (D/'observer.stdout').open('x')as o,(D/'observer.stderr').open('x')as e:
 p=subprocess.Popen(cmd,stdout=o,stderr=e);save('running-experiment.json',dict(pid=p.pid,controller_pid=os.getpid(),command=cmd));pause=root/'r07-provider-pause'
 try:
  waitfor(lambda:(c if c and c['scan']['scan']['iteration']>=2 and c['ack']['state']['epoch']==c['topology']['header']['epoch'] and c['ack']['state']['replaced_bytes']>0 else None)if(c:=context())else None,p)
  idle_context=context();lt=topology(Path(idle_context['topology']['path']),idle_context['ack']['state']['mapped_bytes']);pinned_baseline=lt['r07_extents'];pt=topology(root/'model/normal-pool/vm-regions-000000.jsonl')
  with concurrent.futures.ThreadPoolExecutor(max_workers=2)as pool:captures.extend(pool.map(lambda pair:capture('idle',*pair),[('layout',lt),('pool',pt)]))
  (layout/'vm-start-first').touch(exist_ok=False)
  waitfor(lambda:any(x.get('snapshot_admitted',{}).get('generation')==1 for x in events()),p)
  with concurrent.futures.ThreadPoolExecutor(max_workers=2)as pool:captures.extend(pool.map(lambda pair:capture('inflight',*pair),[('layout',lt),('pool',pt)]))
  (layout/'vm-start-second').touch(exist_ok=False)
  waitfor(lambda:any(x.get('snapshot_admitted',{}).get('generation')==2 for x in events()),p)
  bound=read(ackpath);assert bound['state']['replaced_bytes']>0;bound_context=context();bound_topology=topology(Path(bound_context['topology']['path']),bound['state']['mapped_bytes']);pause.touch(exist_ok=False)
  repaid=waitfor(lambda:(a if a and a['state']['replaced_bytes']==0 and a['permitted_bytes']==0 else None)if(a:=read(ackpath))else None,p,10)
  waitfor(lambda:(c if c and c['topology']['header']['epoch']==repaid['state']['epoch']else None)if(c:=context())else None,p,10)
  filled_context=context();filled=topology(Path(filled_context['topology']['path']),repaid['state']['mapped_bytes'])
  assert repaid['state']['mapped_bytes']==repaid['state']['logical_bytes']-repaid['state']['arena_tail_bytes']
  retained=assert_repaid_identity(bound_topology['r07_extents'],filled['r07_extents'],repaid['state']['mapped_bytes'])
  facts.append(dict(kind='actual_repaid_extent_topology_validated',before=bound,after=repaid,before_extents=bound_topology['r07_extents'],after_extents=filled['r07_extents'],before_extent_count=len(bound_topology['r07_extents']),repaid_extent_count=len(retained),logical_mapped_bytes=filled['r07_mapped_bytes'],topology_path=filled['path'],topology_sha256=filled['sha256'],preserved_full_extents='required',replaced_short_tail_count='0 or 1 with new generation'))
  time.sleep(.8)
 finally:
  pause.unlink(missing_ok=True)
  # Release admission markers only; never signal/stop a model or safety observer.
  if layout.exists():
   (layout/'vm-start-first').touch(exist_ok=True);(layout/'vm-start-second').touch(exist_ok=True)
 code=p.wait(timeout=140)
report=read(root/'observer-report.json');save('terminal.json',dict(source_sha256=info['source_sha256'],binary_sha256=info['binary_sha256'],observer_exit=code,actual_seconds=time.monotonic()-start,captures=captures,facts=facts,report=report,formal_performance='not_accepted',physical_credit_from_VM=0,real16gb='not_run',eight_hour='not_run'))
assert code==0 and report['status']=='passed_measurement_wrapper'
assert all(x['exit_code']==0 and x['identity_unchanged'] and x['stable_phase_epoch_list_endpoints']for x in captures)
# Independently cold-restore both real snapshot generations before any retirement.
env={k:v for k,v in os.environ.items()if not k.startswith('MYCITY_')};cold_results=[]
for generation in (1,2):
 snap=layout/f'snapshots/snapshot-{generation:06d}';cold=W/f'independent-cold-gen{generation}';t=time.monotonic();assert snap.is_dir()
 with (D/f'independent-cold-gen{generation}.stdout').open('x')as o,(D/f'independent-cold-gen{generation}.stderr').open('x')as e:q=subprocess.run([exe,'layout-load-only',str(snap),str(cold)],env=env,stdout=o,stderr=e,timeout=120)
 r=read(cold/'report.json');assert q.returncode==0 and r['cold_reopen_and_scan']=='passed' and r['manifest']['r07']['replaced_bytes']==0
 cold_results.append(dict(generation=generation,exit=q.returncode,seconds=time.monotonic()-t,snapshot=str(snap),report=str(cold/'report.json'),source_sha256=info['source_sha256'],whole_historical_credit_restored=False))
save('cold-result.json',dict(generations=cold_results,all_generations_cold_restored=len(cold_results)==2,source_sha256=info['source_sha256']))
print(json.dumps(dict(observer_exit=code,seconds=time.monotonic()-start,captures=len(captures),all_stable=True,cold_generations=len(cold_results),physical_credit_from_VM=0)),flush=True)
