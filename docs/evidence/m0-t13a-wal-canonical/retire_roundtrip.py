#!/usr/bin/env python3
"""Retire only this stage's proven disposable roundtrip fork, file by file."""
import collections,hashlib,json,os,subprocess,time
from pathlib import Path
D=Path(__file__).resolve().parent;R=D.parents[2];os.chdir(R)
assert json.loads((D/'roundtrip-proof.json').read_text())['exit']==0
assert json.loads((D/'roundtrip-result.json').read_text())['exit']==0
assert not (D/'roundtrip-retirement.json').exists()
root=Path('work/t13a74-wal-roundtrip')
processes=subprocess.check_output(['ps','-axo','pid,ppid,lstart,comm'],text=True)
(D/'retirement-processes.txt').write_text(processes)
assert not any(l.split()[-1].endswith(('mycity-headless','/roundtrip','/compat-restore')) for l in processes.splitlines() if l.split()),'active verification/build; postpone'
p=subprocess.run(['lsof','-nP','-Fpcftn'],text=True,stdout=subprocess.PIPE,stderr=subprocess.PIPE);assert p.returncode==0
(D/'retirement-open-files.txt').write_text('\n'.join(l for l in p.stdout.splitlines() if str(R) in l)+'\n')
opened={l[1:] for l in p.stdout.splitlines() if l.startswith('n')}
assert not any(n.startswith(str(R/'work')+'/') or n.startswith(str(R/'target')+'/') for n in opened), 'occupied project artifact; postpone'
# Other projects Cargo/rustc are allowed; no foreign task is stopped or cleaned.
def stat(p):
 s=p.stat();return [s.st_dev,s.st_ino,s.st_size,s.st_mtime_ns,s.st_blocks*512,s.st_nlink]
allfiles={}
for base,dirs,names in os.walk('work'):
 for name in names:
  path=Path(base)/name;assert not path.is_symlink();allfiles[str(path)]=stat(path)
selected={n:v for n,v in allfiles.items() if Path(n).is_relative_to(root)}
assert selected and all(n.startswith(str(root/'world')+'/') for n in selected)
# These files originated solely in this phase's controlled shutil.copytree + two saves.
rows=[dict(path=n,identity=v,reason='Successful disposable old-world to streamed-checkpoint correctness fork; no independent restore dependency; original and compatibility copy retained',retained_evidence='docs/evidence/m0-t13a-wal-canonical/roundtrip-proof.json') for n,v in sorted(selected.items())]
(D/'roundtrip-retirement-plan.json').write_text(json.dumps(rows,indent=2)+'\n')
groups=collections.defaultdict(list)
for n,v in selected.items():groups[tuple(v[:2])].append(n)
for key,paths in groups.items():
 aliases=[n for n,v in allfiles.items() if tuple(v[:2])==key]
 assert set(aliases)==set(paths) and allfiles[paths[0]][5]==len(paths),'external hardlink'
def metrics():
 s=os.statvfs(R);return dict(unix_ns=time.time_ns(),filesystem_available_bytes=s.f_bavail*s.f_frsize,work_du_kib=int(subprocess.check_output(['du','-sk','work'],text=True).split()[0]),target_du_kib=int(subprocess.check_output(['du','-sk','target'],text=True).split()[0]))
keep={n:v for n,v in allfiles.items() if n not in selected}
before=metrics()
for n,v in selected.items():
 path=Path(n);assert stat(path)==v;path.unlink()
for directory in sorted([p for p in root.rglob('*') if p.is_dir()],key=lambda p:len(p.parts),reverse=True):directory.rmdir()
root.rmdir()
for n,v in keep.items():assert stat(Path(n))==v,n
after=metrics()
r=dict(before=before,after=after,removed_paths=len(selected),removed_unique_inodes=len(groups),removed_unique_allocated_bytes=sum(allfiles[n[0]][4] for n in groups.values()),filesystem_available_delta_bytes=after['filesystem_available_bytes']-before['filesystem_available_bytes'],retained_work_files_checked_unchanged=len(keep),nonselected_inventory_sha256=hashlib.sha256(json.dumps(keep,sort_keys=True).encode()).hexdigest(),availability='Disposable roundtrip fork retired; cannot directly reopen it. Original full world and compatibility copy retained, hashes are not substitutes for them.',accounting='Device/inode deduplicated st_blocks; observed filesystem delta is not wholly attributed. No target/cache deletion.')
(D/'roundtrip-retirement.json').write_text(json.dumps(r,indent=2)+'\n');print(json.dumps(r,indent=2))
