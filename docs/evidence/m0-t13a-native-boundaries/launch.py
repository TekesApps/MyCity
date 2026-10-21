"""Preflight then exec the observer: no parked orchestration parent in measurement."""
import hashlib,json,os,struct,subprocess,sys,time,zipfile
from pathlib import Path
D=Path(__file__).resolve().parent;R=D.parents[2];os.chdir(R)
scenario,mode,name=sys.argv[1:];assert scenario in ('l0','l1','l2') and mode in ('smoke','formal')
assert name.startswith('native-boundaries-') and '/' not in name
info=json.loads((D/'source.json').read_text());W=R/'work'/name;assert not W.exists()
def check_source():
 h=hashlib.sha256()
 with zipfile.ZipFile(D/'frozen-source.zip') as z:
  for n in sorted(z.namelist()):
   b=z.read(n);assert (R/n).read_bytes()==b,n;s=n.encode();h.update(struct.pack('<Q',len(s)));h.update(s);h.update(struct.pack('<Q',len(b)));h.update(b)
 assert h.hexdigest()==info['source_sha256']
 assert hashlib.sha256(Path(info['binary']).read_bytes()).hexdigest()==info['binary_sha256']
check_source();W.mkdir();(W/'measurement-requested').touch()
intent={'run':str(W.relative_to(R)),'scenario':scenario,'mode':mode,'source_sha256':info['source_sha256'],'purpose':'Native actual model and allocator boundary diagnostic' if mode=='smoke' else 'Original formal same-source scenario round','full_snapshot_dependency':mode=='formal','terminal_retirement':'After actual terminal, full latest cold restore/index verification and advisor dependency audit, retire short diagnostic pure pressure/reserve by exact files. Keep source/exes/raw/index/receipts/faults. Formal two originals require actual replacement. No cleanup/build while measurement.'}
(W/'retention-intent.json').write_text(json.dumps(intent,indent=2)+'\n')
started=time.monotonic()
with (D/(name+'-admission.stdout')).open('x') as o,(D/(name+'-admission.stderr')).open('x') as e:
 result=subprocess.run([sys.executable,str(D/'readiness.py'),str(W)],stdout=o,stderr=e)
(D/(name+'-admission.json')).write_text(json.dumps({'exit_code':result.returncode,'seconds':time.monotonic()-started,'pid':os.getpid()},indent=2)+'\n')
if result.returncode:raise SystemExit(result.returncode)
check_source()
for key in ['MYCITY_NORMAL_POOL','MYCITY_BROKER_SESSION','MYCITY_BROKER_SUPERVISOR','MYCITY_NORMAL_WORKER_GRANT','MYCITY_ROLE_CONSTRUCTION_OOM']:os.environ.pop(key,None)
support={p.name:hashlib.sha256(p.read_bytes()).hexdigest() for p in [D/'observe.py',D/'native_metrics.py',D/'launch.py',D/'readiness.py']}
(D/(name+'-support-freeze.json')).write_text(json.dumps(support,indent=2)+'\n')
command=[sys.executable,str(D/'observe.py'),str(W/'observed'),'--',info['binary'],'steady-run',str(W/'observed/model'),scenario,mode]
(D/'actual-live.json').write_text(json.dumps({'pid':os.getpid(),'phase':'exec observer immediately; no parked parent','command':command,'source_sha256':info['source_sha256'],'support_sha256':support},indent=2)+'\n')
os.execv(sys.executable,command)
