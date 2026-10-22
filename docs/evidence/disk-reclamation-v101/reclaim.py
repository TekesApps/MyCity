"""One-shot exact retirement; prepare is read-only, execute requires saved review."""
import gzip, hashlib, json, os, shutil, stat, subprocess, sys, time
from pathlib import Path
R=Path('/Users/frankqin/dev/MyCity'); D=Path(__file__).resolve().parent
os.chdir(R)
def save(name,value):
 p=D/name
 with p.open('w') as f: json.dump(value,f,indent=2);f.write('\n');f.flush();os.fsync(f.fileno())
def sha(p):
 h=hashlib.sha256()
 with p.open('rb') as f:
  for b in iter(lambda:f.read(1024*1024),b''):h.update(b)
 return h.hexdigest()
def meta(p):
 s=p.lstat();return [s.st_dev,s.st_ino,s.st_mode,s.st_size,s.st_mtime_ns,s.st_blocks,s.st_nlink]
def inv():
 rows={}
 for root in ['work','target']:
  for parent,dirs,files in os.walk(root):
   for n in files:
    p=Path(parent)/n;rows[str(p)]=meta(p)
 return rows

def occupancy(label):
 ps=subprocess.check_output(['ps','-axo','pid,ppid,etime,command'],text=True)
 sanitized=['PID PPID ELAPSED EXECUTABLE (arguments omitted)']
 for line in ps.splitlines()[1:]:
  f=line.split(None,3)
  if len(f)==4:sanitized.append(' '.join(f[:3]+[f[3].split()[0]]))
 (D/f'{label}-processes.txt').write_text('\n'.join(sanitized)+'\n')
 active=[]
 for line in ps.splitlines()[1:]:
  fields=line.split(None,3)
  if len(fields)==4:
   exe=fields[3].split()[0].lower()
   if any(x in exe for x in ['mycity','cargo','rustc']):active.append(line)
 assert not active,active
 f=subprocess.run(['lsof','-nP','+D',str(R/'work'),'+D',str(R/'target')],text=True,capture_output=True)
 save(f'{label}-occupancy.json',dict(code=f.returncode,stdout=f.stdout,stderr=f.stderr))
 assert f.returncode==1 and not f.stdout and not f.stderr
 return {'du_kib':{r:int(subprocess.check_output(['du','-sk',r],text=True).split()[0]) for r in ['work','target']},'available_bytes':shutil.disk_usage(R).free,'time_ns':time.time_ns()}
def wip():
 paths=subprocess.check_output(['git','ls-files','--cached','--others','--exclude-standard'],text=True).splitlines()
 return {s:sha(Path(s)) for s in paths if Path(s).is_file() and (s.startswith('crates/') or s in ['Cargo.lock','Cargo.toml'])}

if sys.argv[1]=='prepare':
 assert not (D/'plan.json').exists()
 before=occupancy('prepare'); inventory=inv()
 with gzip.open(D/'inventory-before.json.gz','wt') as f:json.dump(inventory,f,separators=(',',':'))
 save('wip-before.json',wip()); plan=[]
 def add(p,reason,evidence,**extra):
  s=p.lstat();assert stat.S_ISREG(s.st_mode) and s.st_nlink==1
  plan.append(dict(path=str(p),metadata=meta(p),sha256=sha(p),allocated_bytes=s.st_blocks*512,round=p.parts[1] if p.parts[0]=='work' else 'obsolete-v100-build-graph',reason=reason,retained_evidence=evidence,**extra))
 smoke=Path('work/steady-controller-smoke-l2-01/observed/model/layout/snapshots/snapshot-000001')
 for n in ['pressure.bin','work-reserve.bin']:
  add(smoke/n,'synthetic load; warm-up deadline failure before measurement, no payload fault; later formal run supersedes this diagnostic, advisor released original recovery dependency','docs/evidence/m0-t13a-steady-controller; original work/steady-controller-smoke-l2-01 logs, business, pool, index and components',directly_restorable_after=False)
 rounds=[('steady-controller-formal-l2-01',7),('steady-controller-formal-l2-01',8),('runtime-vm-l1-01',1),('causal-clock-formal-l1-01',1),('snapshot-backing-formal-l1-01',1)]
 for run,g in rounds:
  base=Path(f'work/{run}/observed/model/layout/snapshots/snapshot-{g:06}')
  assert not (base/'pressure.bin').exists() and not (base/'work-reserve.bin').exists()
  for n in ['native.json','meter.json']:
   p=base/n;a=Path('work/retained-component-evidence/duplicate-components-v1')/(n+'.gz')
   with p.open('rb') as f,gzip.open(a,'rb') as gzf:
    while True:
     b=f.read(1024*1024);c=gzf.read(1024*1024);assert b==c,(p,a)
     if not b:break
   add(p,'normal retired layout; full bytes exactly equal to preexisting lossless archive; not a pending or negative fixture','docs/evidence/disk-reclamation-duplicate-components; original run/index/logs remain',archive=str(a),archive_sha256=sha(a),index=str(base/'index.json'),index_sha256=sha(base/'index.json'),directly_restorable_after=False)
 for f in Path('target/release/.fingerprint').glob('mycity-headless-*/lib-mycity_headless.json'):
  j=json.loads(f.read_text());deps={x[1] for x in j['deps']}
  if 'libproc' in deps and not {'sysctl','rustix'}<=deps:
   for p in Path('target/release/deps').glob('libmycity_headless-'+f.parent.name.split('-')[-1]+'.*'):
    if p.suffix in ['.rlib','.rmeta']:
     add(p,'old headless static cache lacks now mandatory sysctl/rustix dependencies; current wrapper graph and all executables retained','docs/evidence/m0-t13a-native-reader; work/candidate-binaries; source/lock/exe and raw unchanged',graph=str(f),graph_sha256=sha(f))
 protected=[]
 for s in Path('work').rglob('pressure.bin'):
  if s.parent==smoke:continue
  for p in sorted(s.parent.iterdir()):
   if p.is_file():protected.append(dict(path=str(p),metadata=meta(p),sha256=sha(p)))
 save('protected-before.json',protected);save('plan.json',plan);save('prepare.json',before)
 print(json.dumps({'candidate_files':len(plan),'allocated_bytes':sum(r['allocated_bytes'] for r in plan),'protected_files':len(protected),'before':before},indent=2))
elif sys.argv[1]=='execute':
 assert (D/'advisor-review.md').is_file() and not (D/'result.json').exists()
 plan=json.loads((D/'plan.json').read_text()); inventory=json.loads(gzip.open(D/'inventory-before.json.gz','rt').read())
 assert inv()==inventory,'concurrent work/target changes, re-audit'
 assert wip()==json.loads((D/'wip-before.json').read_text())
 for x in plan:
  p=Path(x['path']);assert meta(p)==x['metadata'] and sha(p)==x['sha256']
  if 'index' in x:
   idx=Path(x['index']);assert sha(idx)==x['index_sha256']
   entry=next(e for e in json.loads(idx.read_text())['files'] if e['name']==p.name)
   assert entry['bytes']==p.stat().st_size and entry['sha256']==x['sha256']
  if 'archive' in x:assert sha(Path(x['archive']))==x['archive_sha256']
 for x in json.loads((D/'protected-before.json').read_text()):
  p=Path(x['path']);assert meta(p)==x['metadata'] and sha(p)==x['sha256']
 before=occupancy('immediate');save('delete-intent.json',plan)
 for x in plan:
  p=Path(x['path']);assert meta(p)==x['metadata'];p.unlink()
 after=occupancy('after'); post=inv();deleted={x['path'] for x in plan}
 assert {p:v for p,v in inventory.items() if p not in deleted}==post
 assert wip()==json.loads((D/'wip-before.json').read_text())
 protected=json.loads((D/'protected-before.json').read_text())
 for x in protected:
  p=Path(x['path']);assert meta(p)==x['metadata'] and sha(p)==x['sha256']
 save('protected-after.json',protected);save('deleted.json',plan)
 result=dict(before=before,after=after,files_deleted=len(plan),allocated_bytes_removed=sum(x['allocated_bytes'] for x in plan),filesystem_available_delta_bytes=after['available_bytes']-before['available_bytes'],other_work_target_files_unchanged=len(post),protected_files_sha_verified=len(protected),source_wip_unchanged=True,archive_added_bytes=0,attribution='st_blocks allocation is not APFS exclusive physical reclaim; df delta observational only, no attribution to deletion or other tasks',scope='no build, no recovery run, no UI/GPU/AgentKit, no performance acceptance')
 save('result.json',result);print(json.dumps(result,indent=2))
