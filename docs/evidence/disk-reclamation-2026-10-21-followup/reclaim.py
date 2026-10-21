#!/usr/bin/env python3
"""One-shot, identity-checked retirement of this dated exact allowlist only."""
import collections,json,os,subprocess,time
from pathlib import Path
R=Path(__file__).resolve().parents[3];D=Path(__file__).resolve().parent
os.chdir(R)
plan=json.loads((D/'deletion-plan.json').read_text());rows=plan['files']
assert not (D/'deletion-result.json').exists(), 'one shot already completed'
processes=subprocess.check_output(['ps','-axo','pid,comm'],text=True)
assert not any(line.split()[-1].endswith('mycity-headless') for line in processes.splitlines() if line.split()), 'model still running'
(D/'processes-before.txt').write_text(processes)
p=subprocess.run(['lsof','-nP','-Fpcn'],text=True,stdout=subprocess.PIPE,stderr=subprocess.PIPE)
assert p.returncode==0, p.stderr
(D/'open-files-before.txt').write_text('\n'.join(line for line in p.stdout.splitlines() if str(R) in line)+'\n')
opened={line[1:] for line in p.stdout.splitlines() if line.startswith('n')}
allfiles={}
for root,dirs,names in os.walk('work'):
 for name in names:
  f=Path(root)/name
  assert not f.is_symlink(),str(f)
  s=f.stat();allfiles[str(f)]=(s.st_dev,s.st_ino,s.st_size,s.st_mtime_ns,s.st_blocks*512,s.st_nlink)
selected={r['path'] for r in rows};assert len(selected)==len(rows)
groups=collections.defaultdict(list)
for r in rows:
 f=Path(r['path']);s=f.stat();assert f.name in ['pressure.bin','work-reserve.bin']
 assert f.is_file() and not f.is_symlink()
 assert not any(str(f)==v or str(f).startswith(v+'/') for v in plan['protected'])
 assert str(f.resolve()) not in opened, 'open file '+str(f)
 assert (s.st_dev,s.st_ino,s.st_size,s.st_mtime_ns)==(r['device'],r['inode'],r['bytes'],r['mtime_ns'])
 groups[(s.st_dev,s.st_ino)].append(r)
for key,rs in groups.items():
 aliases=[n for n,v in allfiles.items() if v[:2]==key]
 assert set(aliases)=={r['path'] for r in rs}
 assert allfiles[aliases[0]][5]==len(aliases), 'unaccounted hardlink'
# All nonselected raw files, including full protected restore roots, must survive unchanged.
keep={n:v for n,v in allfiles.items() if n not in selected}
def metrics():
 s=os.statvfs(R)
 return {'unix_ns':time.time_ns(),'filesystem_available_bytes':s.f_bavail*s.f_frsize,'work_du_kib':int(subprocess.check_output(['du','-sk','work'],text=True).split()[0]),'target_du_kib':int(subprocess.check_output(['du','-sk','target'],text=True).split()[0])}
before=metrics();(D/'pre-delete.json').write_text(json.dumps(before,indent=2)+'\n')
for r in rows:
 f=Path(r['path']);s=f.stat();assert (s.st_dev,s.st_ino,s.st_size,s.st_mtime_ns)==allfiles[str(f)][:4]
 f.unlink()
for n,v in keep.items():
 s=Path(n).stat();assert (s.st_dev,s.st_ino,s.st_size,s.st_mtime_ns,s.st_blocks*512,s.st_nlink)==v,n
assert all(not Path(n).exists() for n in selected)
after=metrics()
result={'before':before,'after':after,'removed_paths':len(rows),'removed_unique_inodes':len(groups),'removed_unique_allocated_bytes':sum(rs[0]['allocated_bytes'] for rs in groups.values()),'removed_unique_logical_bytes':sum(rs[0]['bytes'] for rs in groups.values()),'filesystem_available_delta_bytes':after['filesystem_available_bytes']-before['filesystem_available_bytes'],'work_du_delta_bytes':(before['work_du_kib']-after['work_du_kib'])*1024,'retained_raw_files_checked_unchanged':len(keep),'accounting':'st_blocks deduplicated by device/inode; not exclusive APFS extents. df delta is observed, not attributed entirely to this deletion.','files':rows}
(D/'deletion-result.json').write_text(json.dumps(result,indent=2)+'\n')
print(json.dumps({k:v for k,v in result.items() if k!='files'},indent=2))
