"""Read-only follow-up: no unlink or build; preserve all unclassified artifacts."""
from pathlib import Path
import collections, hashlib, json, os, subprocess, time
R=Path('/Users/frankqin/dev/MyCity'); D=Path(__file__).resolve().parent
os.chdir(R)
def save(n,v): (D/n).write_text(json.dumps(v,indent=2)+'\n')
def metrics():
 s=os.statvfs(R)
 return dict(time_ns=time.time_ns(),work_du_kib=int(subprocess.check_output(['du','-sk','work'],text=True).split()[0]),target_du_kib=int(subprocess.check_output(['du','-sk','target'],text=True).split()[0]),filesystem_available_bytes=s.f_bavail*s.f_frsize)
def sha(p):
 h=hashlib.sha256()
 with p.open('rb') as f:
  for c in iter(lambda:f.read(4*1024*1024),b''):h.update(c)
 return h.hexdigest()
def inventory():
 rows={}
 for root,ds,ns in os.walk('work'):
  for n in ns:
   p=Path(root)/n;s=p.lstat()
   rows[str(p)]=[s.st_dev,s.st_ino,s.st_size,s.st_mtime_ns,s.st_blocks*512,s.st_nlink]
 return rows
before=metrics(); files=inventory()
p=subprocess.check_output(['ps','-axo','pid,ppid,lstart,comm'],text=True)
(D/'processes.txt').write_text(p)
l=subprocess.run(['lsof','-nP','-Fpcftn'],text=True,stdout=subprocess.PIPE,stderr=subprocess.PIPE)
assert l.returncode==0,l.stderr
blocks=l.stdout.split('\np'); related=[]; occupied=[]
for b in blocks:
 if str(R) in b:
  related.append('\n'.join(x for x in b.splitlines() if x[:1] in 'pc' or str(R) in x))
 for x in b.splitlines():
  if x.startswith('n'+str(R/'work')+'/') or x.startswith('n'+str(R/'target')+'/'): occupied.append(x[1:])
(D/'project-open-files.txt').write_text('\n'.join(related)+'\n')
save('process-check.json',dict(occupied_work_or_target=occupied,active_model_processes=[x for x in p.splitlines() if x.split() and x.split()[-1].endswith('mycity-headless')],stopped_processes=[]))
assert not occupied,'no cleanup permitted while artifacts occupied'
protected=[]
for relative in ['work/t13a70-joint-01/model/layout/snapshots/snapshot-000001','work/t13a70-joint-01/model/layout/snapshots/snapshot-000002','work/t13a50-joint-01/model/layout/snapshots/snapshot-000001']:
 root=Path(relative);index=json.loads((root/'index.json').read_text())
 for e in index['files']:
  f=root/e['name']; assert f.stat().st_size==e['bytes'] and sha(f)==e['sha256'],str(f)
 protected.append(dict(path=relative,indexed_files=len(index['files']),complete_originals_present=True,sha256_match=True))
manifest=json.loads(Path('docs/evidence/m0-t13a-model-workspace/legacy-input-manifest.json').read_text())
for root in [Path('work/t13a65-joint-02/model/business/world'),Path('work/t13a68-legacy-reopen/world')]:
 actual={str(f.relative_to(root)) for f in root.rglob('*') if f.is_file()}
 assert actual==set(manifest)
 for name,e in manifest.items():
  f=root/name; assert f.stat().st_size==e['bytes'] and sha(f)==e['sha256'],str(f)
 protected.append(dict(path=str(root),indexed_files=len(manifest),complete_originals_present=True,sha256_match=True))
assert not Path('work/t13a74-wal-roundtrip').exists()
save('protected-inputs.json',protected)
names=collections.defaultdict(lambda:[0,0]);seen=set()
for path,s in files.items():
 key=tuple(s[:2])
 if key not in seen:
  seen.add(key);names[Path(path).name][0]+=1;names[Path(path).name][1]+=s[4]
save('largest-file-groups.json',sorted([dict(name=k,inodes=v[0],allocated_bytes=v[1]) for k,v in names.items()],key=lambda x:-x['allocated_bytes'])[:40])
assert files==inventory(),'work changed during audit'
after=metrics()
save('audit-result.json',dict(before=before,after=after,work_files_checked_unchanged=len(files),new_deleted_paths=0,new_deleted_allocated_bytes=0,observed_filesystem_available_delta=after['filesystem_available_bytes']-before['filesystem_available_bytes'],target_shared=True,build_launched=False,new_target_created=False,reason='Remaining large pressure files are required complete restore inputs; other native/meter components and proof-linked build artifacts not proven redundant. No further deletion authorized by classification.',accounting='Read-only audit. Any df/target movement is not attributed to this audit. Prior retirements are reported separately, not counted again.'))
print(json.dumps(dict(before=before,after=after,files=len(files),protected=len(protected)),indent=2))
