"""One original formal L1 run, shared frozen sources/executable/observer, no restart."""
import hashlib,json,os,struct,subprocess,sys,time,zipfile
from pathlib import Path
F=Path(__file__).resolve().parent;R=F.parents[2];P=F;H=R/'docs/evidence/m0-t13a-causal-clock';os.chdir(R)
info=json.loads((P/'source.json').read_text());freeze=json.loads((P/'freeze.json').read_text())
assert json.loads((P/'regression-terminal.json').read_text())['all_commands_exit_zero']
subprocess.run([sys.executable,str(P/'metadata_bound.py')],cwd=R,check=True)
proof=json.loads((P/'metadata-bound.json').read_text());assert proof['source_sha256']==info['source_sha256'] and max(proof['stage_upper_bytes'].values())<=proof['cap_bytes']
assert json.loads((P/'advisor-metadata-admission.json').read_text())['canonical_external_admission_approved']
W=R/'work/typed-workspace-formal-l1-01';assert not W.exists()
sha=lambda p:hashlib.sha256(Path(p).read_bytes()).hexdigest()
def check():
 h=hashlib.sha256()
 with zipfile.ZipFile(P/'frozen-source.zip') as z:
  for n in sorted(z.namelist()):
   b=z.read(n);assert (R/n).read_bytes()==b,n;s=n.encode();h.update(struct.pack('<Q',len(s)));h.update(s);h.update(struct.pack('<Q',len(b)));h.update(b)
 assert h.hexdigest()==info['source_sha256']
 assert sha(info['binary'])==info['binary_sha256']
 assert sha(P/'frozen-source.zip')==freeze['source_zip_sha256']
 assert sha(R/'fixtures/m0-t13a-acceptance.json')==freeze['acceptance_sha256']
check()
idx=json.loads((H/'retention/retained-snapshot-index.json').read_text());snapshot_bytes=sum(x['bytes'] for x in idx['files']);assert snapshot_bytes==3378784402
expected_peak=3*snapshot_bytes+384*1024**2
s=os.statvfs(R);available=s.f_bavail*s.f_frsize
assert expected_peak<20*1024**3 and available>=expected_peak+2*1024**3
support_paths=[P/n for n in ('observe.py','native_metrics.py','clock_exchange.py','readiness.py','metadata_bound.py','metadata-bound.json','advisor-metadata-admission.json')]+[Path(__file__).resolve()]
support={str(p.relative_to(R)):sha(p) for p in support_paths}
(F/'preflight.json').write_text(json.dumps(dict(source_sha256=info['source_sha256'],source_zip=str((P/'frozen-source.zip').relative_to(R)),fixed_binary=info['binary'],fixed_binary_sha256=info['binary_sha256'],single_snapshot_file_bytes=snapshot_bytes,three_live_generations_and_log_allowance_bytes=expected_peak,filesystem_available_bytes=available,world_max_bytes=20*1024**3,safety_free_bytes=2*1024**3,support_sha256=support,no_new_target=True,original_parameters_unchanged=True,validation_order='full same-source regression and independent canonical metadata preflight before original formal-parameter reproduction; physical/steady gates remain measured, not pre-approved'),indent=2)+'\n')
W.mkdir();(W/'measurement-requested').touch()
(W/'retention-intent.json').write_text(json.dumps(dict(scenario='l1',mode='formal',source_sha256=info['source_sha256'],warmup_seconds=300,measurement_seconds=1800,full_snapshot_dependency=True,purpose='persistent typed-workspace original formal repeated snapshot and fixed-region validation; not overall memory acceptance',retirement='two complete originals retained until actual successor recovery and independent dependency release; old formal7/8,v50,fault pending remain protected; no cleanup/build while run'),indent=2)+'\n')
start=time.monotonic()
with (F/'admission.stdout').open('x') as out,(F/'admission.stderr').open('x') as err:
 code=subprocess.run([sys.executable,str(P/'readiness.py'),str(W)],stdout=out,stderr=err).returncode
(F/'admission.json').write_text(json.dumps(dict(pid=os.getpid(),exit_code=code,seconds=time.monotonic()-start),indent=2)+'\n')
if code:raise SystemExit(code)
check();assert {str(p.relative_to(R)):sha(p) for p in support_paths}==support
for key in ('MYCITY_NORMAL_POOL','MYCITY_BROKER_SESSION','MYCITY_BROKER_SUPERVISOR','MYCITY_NORMAL_WORKER_GRANT','MYCITY_ROLE_CONSTRUCTION_OOM'):os.environ.pop(key,None)
cmd=[sys.executable,str(P/'observe.py'),str(W/'observed'),'--',info['binary'],'steady-run',str(W/'observed/model'),'l1','formal']
(F/'live.json').write_text(json.dumps(dict(pid=os.getpid(),phase='exec observer after actual admission; no parked orchestration parent',command=cmd,source_sha256=info['source_sha256'],support_sha256=support),indent=2)+'\n')
os.execv(sys.executable,cmd)
