"""One-shot exact retirement after real cold restore; never prefix deletes."""
import hashlib,json,os,stat,subprocess,time
from pathlib import Path
D=Path(__file__).resolve().parent;R=D.parents[2];os.chdir(R);W=R/'work';V=W/'runtime-vm-l1-01';O=V/'observed';T=D/'retirement';assert not T.exists();T.mkdir()
def write(n,x):(T/n).write_text(json.dumps(x,indent=2)+'\n')
def sha(p):
 h=hashlib.sha256()
 with p.open('rb') as f:
  for b in iter(lambda:f.read(1024*1024),b''):h.update(b)
 return h.hexdigest()
def meta(p):
 s=p.lstat();return [s.st_dev,s.st_ino,s.st_mode,s.st_size,s.st_mtime_ns,s.st_nlink]
def audit_occupancy():
 r=subprocess.run(['lsof','-nP','+D',str(W),'+D',str(R/'target')],capture_output=True,text=True);assert r.returncode==1 and not r.stdout,(r.returncode,r.stdout,r.stderr)
 return dict(command='lsof -nP +D work +D target',returncode=r.returncode,stdout=r.stdout,stderr=r.stderr,monotonic_ns=time.monotonic_ns())
def disk():
 r=subprocess.run(['du','-sk','work','target'],capture_output=True,text=True,check=True)
 return dict(du_kib={row.split()[1]:int(row.split()[0]) for row in r.stdout.splitlines()},filesystem_available_bytes=os.statvfs(R).f_bavail*os.statvfs(R).f_frsize)
assert json.loads((D/'terminal.json').read_text())['observer_exit']==0
assert json.loads((O/'model/layout-restored/report.json').read_text())['cold_reopen_and_scan']=='passed'
for item in D.glob('*-*-*.json'):
 x=json.loads(item.read_text())
 if 'tool_begin_monotonic_ns' in x:
  try:os.kill(x['pid'],0)
  except ProcessLookupError:pass
  else:raise RuntimeError('old PID exists; stop for identity review')
paths=[O/'model/layout/snapshots/snapshot-000001'/name for name in ['pressure.bin','work-reserve.bin']]
manifest=[]
for p in paths:
 s=p.lstat();assert stat.S_ISREG(s.st_mode) and s.st_nlink==1
 manifest.append(dict(path=str(p.relative_to(R)),run='runtime-vm-l1-01',generation=1,metadata=meta(p),allocated_bytes=s.st_blocks*512,sha256=sha(p),reason='pure synthetic payload, terminal actual readback+cold recovery passed, advisor released all recovery dependencies',retained_evidence='docs/evidence/m0-t13a-runtime-vm; original snapshot index/components and observed raw remain'))
assert [p.stat().st_size for p in paths]==[3044040310,134217728]
# Complete original files remain mandatory, not replaced by hashes.
protected=[]
for generation in [7,8]:
 base=W/f'typed-workspace-formal-l1-01/observed/model/layout/snapshots/snapshot-{generation:06}'
 for p in sorted(base.iterdir()):
  if p.is_file():protected.append((p,meta(p),sha(p)))
assert len(protected)==64
allowed=set(paths);other={str(p.relative_to(R)):meta(p) for p in W.rglob('*') if p.is_file() and p not in allowed}
occupancy=audit_occupancy();before=disk();write('before.json',dict(disk=before,occupancy=occupancy,protected_count=len(protected),other_file_count=len(other)));write('allowlist.json',manifest)
for p,item in zip(paths,manifest):
 assert meta(p)==item['metadata'];p.unlink()
assert all(not p.exists() for p in paths)
assert {str(p.relative_to(R)):meta(p) for p in W.rglob('*') if p.is_file()}==other
for p,m,h in protected:assert meta(p)==m and sha(p)==h
post=audit_occupancy();after=disk()
write('availability.json',[dict(path=str(p.relative_to(R)),state='retired; not directly fully restorable; index/hash do not replace original') for p in paths])
write('result.json',dict(before=before,after=after,deleted_files=len(paths),allocated_bytes_removed=sum(x['allocated_bytes'] for x in manifest),filesystem_available_delta_bytes=after['filesystem_available_bytes']-before['filesystem_available_bytes'],attribution='inode allocated bytes, not APFS exclusive physical bytes; filesystem delta separately observed, not fully credited',all_other_work_file_metadata_unchanged=True,protected_current_files_rehashed=len(protected),source_exe_business_wal_raw_preserved=True,target_reused=True,post_occupancy=post))
print((T/'result.json').read_text())
