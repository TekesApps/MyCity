"""One-shot exact retirement; no directory-prefix deletion, no model/build execution."""
import gzip,hashlib,json,os,shutil,stat,subprocess,sys,time
from pathlib import Path
R=Path('/Users/frankqin/dev/MyCity');D=Path(__file__).resolve().parent
os.chdir(R)
def save(n,v):
 with (D/n).open('w') as f:json.dump(v,f,indent=2);f.write('\n');f.flush();os.fsync(f.fileno())
def sha(p):
 h=hashlib.sha256()
 with Path(p).open('rb') as f:
  for b in iter(lambda:f.read(1024*1024),b''):h.update(b)
 return h.hexdigest()
def meta(p):
 s=Path(p).lstat();return [s.st_dev,s.st_ino,s.st_mode,s.st_size,s.st_mtime_ns,s.st_blocks,s.st_nlink]
def inv():
 return {str(Path(a)/n):meta(Path(a)/n) for root in ['work','target'] for a,_,files in os.walk(root) for n in files}
def wip():
 names=subprocess.check_output(['git','ls-files','--cached','--others','--exclude-standard'],text=True).splitlines()
 return {p:sha(p) for p in names if Path(p).is_file() and (p.startswith('crates/') or p in ['Cargo.toml','Cargo.lock'])}
def occupancy(label):
 ps=subprocess.check_output(['ps','-axo','pid,ppid,etime,command'],text=True)
 rows=[];active=[]
 for l in ps.splitlines()[1:]:
  f=l.split(None,3)
  if len(f)!=4:continue
  exe=f[3].split()[0];rows.append(' '.join(f[:3]+[exe]))
  if any(s in Path(exe).name.lower() for s in ['mycity','cargo','rustc']):active.append(l)
 (D/(label+'-processes.txt')).write_text('\n'.join(rows)+'\n');assert not active,active
 p=subprocess.run(['lsof','-nP','+D',str(R/'work'),'+D',str(R/'target')],capture_output=True,text=True)
 save(label+'-occupancy.json',dict(code=p.returncode,stdout=p.stdout,stderr=p.stderr))
 assert p.returncode==1 and not p.stdout and not p.stderr,'occupied or incomplete lsof'
 return dict(time_ns=time.time_ns(),du_kib={p:int(subprocess.check_output(['du','-sk',p],text=True).split()[0]) for p in ['work','target']},available_bytes=shutil.disk_usage(R).free)
def protected():
 paths={x['path'] for x in json.loads(Path('docs/evidence/disk-reclamation-v101/protected-after.json').read_text())}
 for p in Path('work').rglob('pressure.bin'):
  paths.update(str(x) for x in p.parent.iterdir() if x.is_file())
 return [dict(path=p,metadata=meta(p),sha256=sha(p)) for p in sorted(paths)]
def check_protected(rows):
 for x in rows:assert meta(x['path'])==x['metadata'] and sha(x['path'])==x['sha256'],x['path']
if sys.argv[1]=='prepare':
 assert not (D/'plan.json').exists()
 before=occupancy('prepare');inventory=inv();plan=[]
 def add(p,reason,evidence,**extras):
  m=meta(p);assert stat.S_ISREG(m[2]) and m[6]==1
  plan.append(dict(path=str(p),metadata=m,sha256=sha(p),allocated_bytes=m[5]*512,round=p.parts[1],reason=reason,retained_evidence=evidence,**extras))
 p=Path('work/r07-backing-preflight-01/r07-prefix.bin');j=json.loads((p.parent/'report.json').read_text())
 assert sha(p)==j['prefix_file']['sha256'] and p.stat().st_size==j['prefix_file']['bytes']
 add(p,'completed same-process synthetic prefix readback/reconstruction; no downstream original-path dependency; advisor released','work/r07-backing-preflight-01/report.json; docs/evidence/m0-t13a-r07-mapping; work/candidate-binaries/r07-mapping',directly_reusable_after=False)
 for run in ['native-observer-l1-01','native-observer-l1-02','steady-controller-smoke-l2-01']:
  b=Path('work')/run/'observed/model/layout/snapshots/snapshot-000001'
  assert not (b/'pressure.bin').exists() and not (b/'work-reserve.bin').exists()
  idx=json.loads((b/'index.json').read_text())
  for n in ['native.json','meter.json']:
   p=b/n;a=Path('work/retained-component-evidence/duplicate-components-v1')/(n+'.gz')
   with p.open('rb') as f,gzip.open(a,'rb') as g:
    while True:
     x=f.read(1024*1024);y=g.read(1024*1024);assert x==y,(p,a)
     if not x:break
   entry=next(x for x in idx['files'] if x['name']==n)
   assert entry['bytes']==p.stat().st_size and entry['sha256']==sha(p)
   add(p,'normal retired synthetic layout; complete bytes match existing lossless archive and original index; no restore dependency','original run/index/raw/report; docs/evidence/m0-t13a-native-observer-errno; docs/evidence/disk-reclamation-v101',archive=str(a),archive_sha256=sha(a),index=str(b/'index.json'),index_sha256=sha(b/'index.json'),directly_restorable_after=False)
 with gzip.open(D/'inventory-before.json.gz','wt') as f:json.dump(inventory,f,separators=(',',':'))
 save('wip-before.json',wip());save('protected-before.json',protected());save('plan.json',plan);save('prepare.json',before)
 print(json.dumps(dict(files=len(plan),allocated_bytes=sum(x['allocated_bytes'] for x in plan),before=before),indent=2))
elif sys.argv[1]=='execute':
 assert (D/'advisor-review.md').is_file() and not (D/'result.json').exists()
 plan=json.loads((D/'plan.json').read_text());inventory=json.loads(gzip.open(D/'inventory-before.json.gz','rt').read());pr=json.loads((D/'protected-before.json').read_text());wp=json.loads((D/'wip-before.json').read_text())
 assert inv()==inventory;assert wip()==wp;check_protected(pr)
 for x in plan:
  assert meta(x['path'])==x['metadata'] and sha(x['path'])==x['sha256']
  if 'archive' in x:assert sha(x['archive'])==x['archive_sha256'] and sha(x['index'])==x['index_sha256']
 before=occupancy('immediate');save('delete-intent.json',plan)
 for x in plan:
  p=Path(x['path']);assert meta(p)==x['metadata'];p.unlink()
 after=occupancy('after');post=inv();gone={x['path'] for x in plan}
 assert {p:v for p,v in inventory.items() if p not in gone}==post
 assert wip()==wp;check_protected(pr);save('protected-after.json',pr);save('deleted.json',plan)
 save('availability.json',dict(prefix_original_removed=True,prefix_probe_scope='same-process only; no original file for direct reuse',retired_layouts_not_directly_restorable=['native-observer-l1-01','native-observer-l1-02','steady-controller-smoke-l2-01'],current_r07_coupling_full_original_retained=True,five_old_complete_or_fault_originals_retained=True,pending_not_valid_restore_point=True))
 result=dict(before=before,after=after,files_deleted=len(plan),allocated_bytes_removed=sum(x['allocated_bytes'] for x in plan),filesystem_available_delta_bytes=after['available_bytes']-before['available_bytes'],other_work_target_files_metadata_unchanged=len(post),protected_files_sha_verified=len(pr),source_and_wip_unchanged=True,new_archive_bytes=0,build_or_model_run=False,attribution='inode-deduplicated st_blocks; not APFS exclusive-block claim; filesystem availability delta observational only')
 save('result.json',result);print(json.dumps(result,indent=2))
