"""Terminal-only exact retirement. Generated inputs only, no business/target cleanup."""
from pathlib import Path
import os,json,hashlib,subprocess,time
D=Path(__file__).resolve().parent;R=D.parents[2];os.chdir(R)
def save(n,v):(D/n).write_text(json.dumps(v,indent=2)+'\n')
def sha(p):
 h=hashlib.sha256()
 with Path(p).open('rb') as f:
  for b in iter(lambda:f.read(4*1024*1024),b''):h.update(b)
 return h.hexdigest()
def metrics():
 s=os.statvfs(R)
 return dict(time_ns=time.time_ns(),work_du_kib=int(subprocess.check_output(['du','-sk','work'],text=True).split()[0]),target_du_kib=int(subprocess.check_output(['du','-sk','target'],text=True).split()[0]),filesystem_available_bytes=s.f_bavail*s.f_frsize)
assert (D/'advisor-final.md').exists() and (D/'store-boundary-proof.json').exists()
raw=subprocess.check_output(['lsof','-nP','-Fpcftn'],text=True)
occupied=[s[1:] for s in raw.splitlines() if s.startswith('n'+str(R/'work')+'/') or s.startswith('n'+str(R/'target')+'/')]
ps=subprocess.check_output(['ps','-axo','pid,ppid,etime,comm'],text=True)
(D/'retention-processes.txt').write_text(ps)
save('retention-process-check.json',dict(occupied_work_or_target=occupied,stopped_processes=[],no_model_or_build_started=True))
assert not occupied,'Never clean while any work/target file is open'
assert not any(s.split()[-1].endswith('/mycity-headless') for s in ps.splitlines() if s.split())
allowed={row['path']:row['sha256'] for row in json.loads((D/'parser-input-manifest.json').read_text())}
world=json.loads((D/'store-boundary-input.json').read_text())
allowed.update({str(Path(world['synthetic_world'])/n):v['sha256'] for n,v in world['files'].items()})
root=R/'work/t13a-lexical-parser-inputs'
assert {str(p.relative_to(R)) for p in root.rglob('*') if p.is_file()}==set(allowed)
# Verify required complete originals before retirement; no hash substitutes for their files.
protected=[]
for rel in ['work/t13a70-joint-01/model/layout/snapshots/snapshot-000001','work/t13a70-joint-01/model/layout/snapshots/snapshot-000002','work/t13a50-joint-01/model/layout/snapshots/snapshot-000001']:
 p=R/rel;idx=json.loads((p/'index.json').read_text())
 for e in idx['files']:assert (p/e['name']).stat().st_size==e['bytes'] and sha(p/e['name'])==e['sha256']
 protected.append(dict(path=rel,indexed_files=len(idx['files']),complete_originals_present=True,sha256_match=True))
for n,h in world['base_sha256'].items():assert sha(R/world['base']/n)==h
save('retention-protected-inputs.json',dict(layouts=protected,original_v1_complete_and_unchanged=True,new_30day_world_retained='work/t13a-role-lexical-business-01/world'))
before=metrics();rows=[]
for name,h in sorted(allowed.items()):
 p=R/name;assert p.is_relative_to(root) and not p.is_symlink();s=p.stat();assert s.st_nlink==1 and p.is_file() and sha(p)==h
 rows.append(dict(path=name,dev=s.st_dev,inode=s.st_ino,logical_bytes=s.st_size,allocated_bytes=s.st_blocks*512,hardlinks=s.st_nlink,mtime_ns=s.st_mtime_ns,sha256=h,round='fixed-lexical-v0.5.83',reason='Synthetic parser pressure or patched v1 copy, probes terminal, no remaining recovery dependency; retained original v1 world unchanged',evidence='docs/evidence/m0-t13a-fixed-lexical/{parser-heap-results.json,store-boundary-proof.json,parser-input-manifest.json,store-boundary-input.json}; source generators/stdout/fixed executables retained',direct_recovery_after_retirement=False))
save('retention-deletion-manifest.json',dict(state='checked_before_unlink',files=rows))
# Recheck global occupation immediately before exact unlinks, never select by t13 prefix.
raw=subprocess.check_output(['lsof','-nP','-Fpcftn'],text=True)
assert not any(s.startswith('n'+str(root)+'/') for s in raw.splitlines())
for r in rows:
 p=R/r['path'];s=p.stat();assert (s.st_dev,s.st_ino,s.st_mtime_ns,s.st_nlink)==(r['dev'],r['inode'],r['mtime_ns'],1);p.unlink()
# Remove only now-empty generated directories, not any protected layout directory.
for p in sorted([p for p in root.rglob('*') if p.is_dir()],key=lambda p:len(p.parts),reverse=True):p.rmdir()
root.rmdir()
after=metrics()
save('retention-deletion-manifest.json',dict(state='retired',files=rows))
save('retention-result.json',dict(before=before,after=after,files_deleted=len(rows),deleted_inode_deduplicated_allocated_bytes=sum(r['allocated_bytes'] for r in rows),target_files_deleted=0,new_independent_target=False,new_l1_copy=False,temporary_input_direct_recovery=False,all_probes_terminal=True,observed_filesystem_available_delta=after['filesystem_available_bytes']-before['filesystem_available_bytes'],accounting='File allocation st_blocks*512, one link per inode checked; not APFS exclusive physical bytes. du includes directories. df independent, not attributed as freed bytes. Full required originals retained, synthetic path not directly restorable.'))
print(json.dumps(json.loads((D/'retention-result.json').read_text()),indent=2))
