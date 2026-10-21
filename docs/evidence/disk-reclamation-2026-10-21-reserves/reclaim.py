#!/usr/bin/env python3
"""One-shot exact six-file retirement; refuses occupied files or changed identities."""
import collections, gzip, hashlib, json, os, subprocess, time
from pathlib import Path
D=Path(__file__).resolve().parent
R=D.parents[2]
os.chdir(R)
assert not (D/'deletion-result.json').exists(), 'already executed'
def save(name,obj):
 (D/name).write_text(json.dumps(obj,indent=2)+'\n')
def statrow(p):
 s=p.stat(); return [s.st_dev,s.st_ino,s.st_size,s.st_mtime_ns,s.st_blocks*512,s.st_nlink]
def inventory():
 result={}
 for root,dirs,names in os.walk('work'):
  for name in names:
   p=Path(root)/name
   assert not p.is_symlink(),str(p)
   result[str(p)]=statrow(p)
 return result
def metrics():
 s=os.statvfs(R)
 return dict(unix_ns=time.time_ns(),filesystem_available_bytes=s.f_bavail*s.f_frsize,
  work_du_kib=int(subprocess.check_output(['du','-sk','work'],text=True).split()[0]),
  target_du_kib=int(subprocess.check_output(['du','-sk','target'],text=True).split()[0]))
protected=[
 'work/t13a70-joint-01',
 'work/t13a50-joint-01/model/layout/snapshots/snapshot-000001',
 'work/t13a65-joint-02/model/business',
 'work/t13a68-legacy-reopen',
 'work/accepted-binaries','work/candidate-binaries']
# Exact paths, not prefix deletion.
rows=[]
for run,evidence in [
 ('t13a64-joint-01','docs/evidence/m0-t13a-hotpath'),
 ('t13a65-joint-01','docs/evidence/m0-t13a-helper-compare/failure-notes.md'),
 ('t13a65-joint-02','docs/evidence/m0-t13a-helper-compare/t13a65-joint-02')]:
 for gen in [1,2]:
  p=Path('work')/run/f'model/layout/snapshots/snapshot-{gen:06d}/work-reserve.bin'
  assert not p.with_name('pressure.bin').exists()
  v=statrow(p); assert v[2]==134217728 and v[5]==1
  rows.append(dict(path=str(p),identity=v,run=run,reason='Obsolete synthetic reserve; pressure already retired; superseded by complete v70 baseline; no future restore dependency.',retained_evidence=evidence,availability_after='Already incomplete layout remains NOT directly full-restorable; native/meter/index/fault evidence retained.'))
save('deletion-plan.json',dict(protected=protected,files=rows))
processes=subprocess.check_output(['ps','-axo','pid,ppid,lstart,comm'],text=True)
(D/'processes-before.txt').write_text(processes)
assert not any(l.split()[-1].endswith('mycity-headless') for l in processes.splitlines() if l.split()),'active model'
p=subprocess.run(['lsof','-nP','-Fpcftn'],text=True,stdout=subprocess.PIPE,stderr=subprocess.PIPE)
assert p.returncode==0,p.stderr
# Include process identities with relevant cwd/file descriptors; omit unrelated file names.
blocks=p.stdout.split('\np')
(D/'open-files-before.txt').write_text('\n'.join('\n'.join(l for l in b.splitlines() if l[:1] in 'pc' or str(R) in l) for b in blocks if str(R) in b)+'\n')
opened={l[1:] for l in p.stdout.splitlines() if l.startswith('n')}
for name in opened:
 if name.startswith(str(R/'target')+'/') or name.startswith(str(R/'work')+'/'):
  raise AssertionError('active project artifact '+name)
files=inventory(); selected={r['path'] for r in rows}; assert len(selected)==6
groups=collections.defaultdict(list)
for row in rows:
 path=row['path'];v=files[path]
 assert v==row['identity']
 assert not any(path==q or path.startswith(q+'/') for q in protected)
 assert str((R/path).resolve()) not in opened
 groups[tuple(v[:2])].append(path)
for key,paths in groups.items():
 aliases=[n for n,v in files.items() if tuple(v[:2])==key]
 assert set(aliases)==set(paths) and files[paths[0]][5]==len(paths),'unaccounted hardlink'
with gzip.open(D/'work-inventory-before.json.gz','wt') as f: json.dump(files,f)
# Validate complete protected layout contents against original indexes, not hashes as substitutes.
verified=[]
for root in [Path(protected[0])/'model/layout/snapshots/snapshot-000001',Path(protected[0])/'model/layout/snapshots/snapshot-000002',Path(protected[1])]:
 index=json.loads((root/'index.json').read_text())
 for e in index['files']:
  path=root/e['name']; h=hashlib.sha256()
  with path.open('rb') as f:
   for chunk in iter(lambda:f.read(4*1024*1024),b''): h.update(chunk)
  assert path.stat().st_size==e['bytes'] and h.hexdigest()==e['sha256'],str(path)
 verified.append(dict(path=str(root),indexed_files=len(index['files']),all_bytes_present_and_sha256_match=True))
save('protected-layout-check.json',verified)
# Freeze baseline immediately before unlink; retain all nonselected files exactly.
before=metrics();save('pre-delete.json',before)
for row in rows:
 path=Path(row['path']); assert statrow(path)==row['identity'];path.unlink()
for name,v in files.items():
 if name not in selected: assert statrow(Path(name))==v,name
for name in selected: assert not Path(name).exists()
after=metrics()
result=dict(before=before,after=after,removed_paths=len(rows),removed_unique_inodes=len(groups),
 removed_unique_allocated_bytes=sum(files[v[0]][4] for v in groups.values()),
 removed_unique_logical_bytes=sum(files[v[0]][2] for v in groups.values()),
 filesystem_available_delta_bytes=after['filesystem_available_bytes']-before['filesystem_available_bytes'],
 work_du_delta_bytes=(before['work_du_kib']-after['work_du_kib'])*1024,
 retained_work_files_stat_unchanged=len(files)-len(selected),files=rows,
 accounting='Device/inode deduplicated st_blocks. Not exclusive APFS extents; observed df delta is not fully attributable to deletion. Shared target retained, no cache deleted.')
save('deletion-result.json',result)
print(json.dumps({k:v for k,v in result.items() if k!='files'},indent=2))
