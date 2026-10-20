#!/usr/bin/env python3
"""Same Rust full business plus real pool, NO L1. Never signs joint pressure/performance."""
import gc,hashlib,json,os,signal,subprocess,sys,time,tracemalloc
from pathlib import Path
ROOT=Path(sys.argv[1]);BINARY=Path(sys.argv[2]).resolve();INFO=Path(__file__).resolve().parent/'source.json';ROOT.mkdir();tracemalloc.start();begin=time.monotonic();canceled=False

def stop(sig,frame):
 global canceled
 canceled=True
signal.signal(signal.SIGTERM,stop)
def sha(p):
 h=hashlib.sha256()
 with p.open('rb') as f:
  while chunk:=f.read(65536):h.update(chunk)
 return h.hexdigest()
def read(p,bound=8192):
 with p.open('rb') as f:data=f.read(bound+1)
 if len(data)>bound:raise RuntimeError('runner_control_bound')
 return json.loads(data)
def save(p,value):
 with p.open('x') as f:json.dump(value,f,indent=2);f.write('\n');f.flush();os.fsync(f.fileno())
def check_hot():
 current,peak=tracemalloc.get_traced_memory();meta=tracemalloc.get_tracemalloc_memory()
 if peak+meta>1024**2:raise RuntimeError('runner_hot_trace_cap')
 return {'current_bytes':current,'peak_bytes':peak,'trace_metadata_bytes':meta,'charged_hot_bytes':peak+meta,'cap_bytes':1024**2,'scope':'runtime hot Python traced data after imports; interpreter/startup/native not included; not whole process heap'}
def wait(child,timeout):
 deadline=time.monotonic()+timeout
 while child.poll() is None:
  if canceled:raise RuntimeError('runner_canceled')
  if time.monotonic()>deadline:raise RuntimeError('runner_child_deadline')
  check_hot();time.sleep(.125)
 return child.returncode
info=read(INFO);rust_sha=sha(BINARY);runner_sha=sha(Path(__file__));assert rust_sha==info['binary_sha256']
save(ROOT/'manifest.json',{'schema':'mycity-pool-full-functional-launch-v1','source_sha256':info['source_sha256'],'rust_binary_sha256':rust_sha,'runner_sha256':runner_sha,'external_owned_charge_bytes':6*1024**2,'scope':'full original business and 64MiB pool; NO L1; extra Python runner; not joint/performance/normal-owned acceptance','candidate_diagnostic_reservations':'Sim2.5 worker1 driver1 helper0.25 pool0.25 Python runtime hot1 =6MiB; observer separate2; Python interpreter/startup/native unclosed'})
pool=None;business=None;status='failed';reason=None
try:
 with (ROOT/'pool.stdout').open('xb') as out,(ROOT/'pool.stderr').open('xb') as err:
  pool=subprocess.Popen([str(BINARY),'normal-pool-host',str(ROOT/'normal-pool')],stdin=subprocess.PIPE,stdout=out,stderr=err,env={**os.environ,'MYCITY_HEAP_LOG':str(ROOT/'heaps.jsonl')})
  deadline=time.monotonic()+10
  while True:
   if pool.poll() is not None:raise RuntimeError('pool_exited_before_ready')
   try:q=read(ROOT/'normal-pool/ready.json')
   except (FileNotFoundError,json.JSONDecodeError):q=None
   if q:
    if q['pid']!=pool.pid or q['source']!=info['source_sha256'] or q['capacity_bytes']!=64*1024**2 or q['fault_injection'] is not None:raise RuntimeError('pool_ready_identity')
    break
   if canceled or time.monotonic()>deadline:raise RuntimeError('pool_ready_deadline')
   check_hot();time.sleep(.025)
  with (ROOT/'business.stdout').open('xb') as bo,(ROOT/'business.stderr').open('xb') as be:
   business=subprocess.Popen([str(BINARY),'scale-sim-probe',str(ROOT/'business'),'full','measure'],stdout=bo,stderr=be,env={**os.environ,'MYCITY_HEAP_LOG':str(ROOT/'heaps.jsonl'),'MYCITY_NORMAL_POOL':str(ROOT/'normal-pool')})
   if wait(business,600)!=0:raise RuntimeError('business_failed')
  # Do not close the owner pipe until the entire real business process exited.
  pool.stdin.close()
  if wait(pool,10)!=0:raise RuntimeError('pool_failed_or_unreaped_grant')
 q=read(ROOT/'normal-pool/report.json');s=q['summary']
 if not q['balanced_after_owned_wait'] or s['available_touched_bytes']!=64*1024**2 or s['granted_capacity_bytes'] or not(s['grants']==s['returns']==s['fill_acknowledgements']==724):raise RuntimeError('pool_complete_balance_mismatch')
 if sha(BINARY)!=rust_sha or sha(Path(__file__))!=runner_sha:raise RuntimeError('runner_or_Rust_binary_changed')
 status='completed_functional_only'
except BaseException as e:
 reason=str(e)
finally:
 for child in [business,pool]:
  if child is not None:
   if child is pool and child.stdin and not child.stdin.closed:child.stdin.close()
   if child.poll() is None:
    child.terminate()
    try:child.wait(timeout=5)
    except subprocess.TimeoutExpired:child.kill();child.wait(timeout=5)
 gc.collect();hot=check_hot()
 save(ROOT/'runner-report.json',{'status':status,'reason':reason,'source_sha256':info['source_sha256'],'rust_binary_sha256':rust_sha,'runner_sha256':runner_sha,'runner_pid':os.getpid(),'pool_pid':pool.pid if pool else None,'business_pid':business.pid if business else None,'actual_wall_seconds':time.monotonic()-begin,'hot_trace':hot,'Rust_binary_unchanged':sha(BINARY)==rust_sha,'runner_unchanged':sha(Path(__file__))==runner_sha,'L1':'absent_independent_functional_branch','shared_8MiB_joint_gate':'not_accepted','exact_normal_owned':'not_accepted','performance':'not_accepted','formal_steady':'not_run','four_speeds':'not_run','eight_hour':'not_run','real_16GB':'not_run'})
if status!='completed_functional_only':raise SystemExit(1)
