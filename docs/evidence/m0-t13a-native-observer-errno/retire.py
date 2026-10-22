"""Retire only terminal native-observer synthetic payloads and obsolete static caches.
No execution allowed before the same-source full regression completes.
"""
import gzip, hashlib, json, os, shutil, stat, subprocess, sys, time
from pathlib import Path
R=Path('/Users/frankqin/dev/MyCity'); D=Path(__file__).resolve().parent/'retirement'
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


P=D.parent
terminal=json.loads((P/'regression-terminal.json').read_text())
assert terminal['all_commands_exit_zero'] and terminal['fixed_binary_unchanged']
source=json.loads((P/'source.json').read_text())
assert terminal['source_sha256']==source['source_sha256']
assert sha(Path(source['binary']))==source['binary_sha256']
if sys.argv[1]=='prepare':
 assert not D.exists();D.mkdir()
 before=occupancy('prepare');inventory=inv()
 with gzip.open(D/'inventory-before.json.gz','wt') as f:json.dump(inventory,f,separators=(',',':'))
 save('wip-before.json',wip());plan=[]
 # The precise rounds were independently released. Never generalize to t13*.
 roots=[Path('work/native-observer-l1-01/observed/model/layout/snapshots'),Path('work/native-observer-l1-02/observed/model/layout/snapshots')]
 roots += [Path('work/native-observer-faults-01')/name/'model/layout/snapshots' for name in ['initializing','worker-compute','worker-compute-l2','snapshot-write','worker-active-l2']]
 for base in roots:
  for p in sorted(base.glob('snapshot-*/pressure.bin'))+sorted(base.glob('snapshot-*/work-reserve.bin')):
   s=p.lstat();assert stat.S_ISREG(s.st_mode) and s.st_nlink==1
   digest=sha(p);index=p.parent/'index.json'
   if index.exists():
    entry=next(x for x in json.loads(index.read_text())['files'] if x['name']==p.name)
    assert entry['bytes']==s.st_size and entry['sha256']==digest
   if 'native-observer-l1-' in str(p):
    cold=json.loads((base.parents[1]/'layout-restored/report.json').read_text())
    assert cold['cold_reopen_and_scan']=='passed'
   maximum=3044040310 if p.name=='pressure.bin' else 134217728
   assert 0<s.st_size<=maximum
   plan.append(dict(path=str(p),metadata=meta(p),sha256=digest,allocated_bytes=s.st_blocks*512,round=str(base),snapshot=str(p.parent),reason='terminal synthetic diagnostic payload; smoke actually cold-restored, owner-loss payload only proves termination and is not a future crash-recovery input; advisor dependency release',retained_evidence='original typed components/index, all raw/source/lock/exe and owner-fault-results/partial CPU targets retained',directly_restorable_after=False,index_sha256=sha(index) if index.exists() else None))
 for f in Path('target/release/.fingerprint').glob('mycity-headless-*/lib-mycity_headless.json'):
  deps={x[1] for x in json.loads(f.read_text())['deps']}
  if {'libproc','sysctl','rustix'}<=deps and 'errno' not in deps:
   for p in Path('target/release/deps').glob('libmycity_headless-'+f.parent.name.split('-')[-1]+'.*'):
    if p.suffix in ['.rlib','.rmeta']:
     s=p.lstat();assert stat.S_ISREG(s.st_mode) and s.st_nlink==1
     plan.append(dict(path=str(p),metadata=meta(p),sha256=sha(p),allocated_bytes=s.st_blocks*512,round='superseded native observer before direct errno dependency',reason='pure regenerable static cache graph lacks now mandatory direct errno dependency; not used by any current feature/profile graph',retained_evidence='all fixed old/final source/lock/exe, original failures and raw preserved',graph=str(f),graph_sha256=sha(f)))
 protected=json.loads(Path('docs/evidence/disk-reclamation-v101/protected-after.json').read_text())
 for x in protected:
  p=Path(x['path']);assert meta(p)==x['metadata'] and sha(p)==x['sha256']
 save('protected-before.json',protected);save('plan.json',plan);save('prepare.json',before)
 print(json.dumps(dict(files=len(plan),allocated_bytes=sum(x['allocated_bytes'] for x in plan),protected_files=len(protected),before=before),indent=2))
elif sys.argv[1]=='execute':
 assert (P/'advisor-review-pending-regression.md').exists() and not (D/'result.json').exists()
 plan=json.loads((D/'plan.json').read_text());inventory=json.loads(gzip.open(D/'inventory-before.json.gz','rt').read())
 assert inv()==inventory,'concurrent mutation; do not delete'
 assert wip()==json.loads((D/'wip-before.json').read_text())
 for x in plan:
  p=Path(x['path']);assert meta(p)==x['metadata'] and sha(p)==x['sha256']
  if 'graph' in x:assert sha(Path(x['graph']))==x['graph_sha256']
  if x.get('index_sha256'):assert sha(p.parent/'index.json')==x['index_sha256']
 before=occupancy('immediate');save('delete-intent.json',plan)
 for x in plan:
  p=Path(x['path']);assert meta(p)==x['metadata'];p.unlink()
 after=occupancy('after');post=inv();removed={x['path'] for x in plan}
 assert post=={p:m for p,m in inventory.items() if p not in removed}
 assert wip()==json.loads((D/'wip-before.json').read_text())
 protected=json.loads((D/'protected-before.json').read_text())
 for x in protected:
  p=Path(x['path']);assert meta(p)==x['metadata'] and sha(p)==x['sha256']
 save('protected-after.json',protected);save('deleted.json',plan)
 availability=[dict(path=x['path'],snapshot=x.get('snapshot'),directly_restorable=False,reason=x['reason'],retained_evidence=x['retained_evidence']) for x in plan if x['path'].startswith('work/')]
 save('availability.json',availability)
 result=dict(before=before,after=after,files_deleted=len(plan),allocated_bytes_removed=sum(x['allocated_bytes'] for x in plan),filesystem_available_delta_bytes=after['available_bytes']-before['available_bytes'],other_work_target_files_unchanged=len(post),protected_files_sha_verified=len(protected),source_wip_unchanged=True,archive_added_bytes=0,attribution='inode allocation is not APFS exclusive physical reclaim; df observed separately; no other task attribution',scope='terminal native observer diagnostics only; no new recovery baseline or pressure copy; target reused')
 save('result.json',result);print(json.dumps(result,indent=2))
