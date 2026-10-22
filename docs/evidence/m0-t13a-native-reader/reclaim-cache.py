"""Retire only old headless library caches whose graph predates mandatory macOS libproc."""
import hashlib,json,os,stat,subprocess,time
from pathlib import Path
D=Path(__file__).resolve().parent;R=D.parents[2];os.chdir(R);C=D/'cache-retirement';assert not C.exists();C.mkdir()
def sha(p):return hashlib.sha256(p.read_bytes()).hexdigest()
def metadata(p):
 s=p.stat();return [s.st_dev,s.st_ino,s.st_mode,s.st_size,s.st_mtime_ns,s.st_nlink]
def save(name,data):(C/name).write_text(json.dumps(data,indent=2)+'\n')
def no_users():
 r=subprocess.run(['lsof','-nP','+D',str(R/'work'),'+D',str(R/'target')],capture_output=True,text=True);assert r.returncode==1 and not r.stdout,(r.returncode,r.stdout,r.stderr)
 p=subprocess.run(['ps','-axo','pid=,comm='],capture_output=True,text=True,check=True);checks=[]
 for row in p.stdout.splitlines():
  parts=row.strip().split(None,1)
  if len(parts)!=2 or Path(parts[1]).name not in ['cargo','rustc','mycity-headless']:continue
  pid=int(parts[0]);q=subprocess.run(['lsof','-a','-p',str(pid),'-d','cwd','-Fn'],capture_output=True,text=True);cwd=[x[1:] for x in q.stdout.splitlines() if x.startswith('n')]
  if not cwd:
   try:os.kill(pid,0)
   except ProcessLookupError:checks.append(dict(pid=pid,state='exited before cwd read'));continue
   raise RuntimeError('cannot attribute live build process '+str(pid))
  assert all(not Path(x).is_relative_to(R) for x in cwd),(pid,cwd)
  checks.append(dict(pid=pid,cwd=cwd,not_this_project=True))
 return dict(lsof_returncode=r.returncode,processes=checks)
def disk():
 r=subprocess.run(['du','-sk','work','target'],capture_output=True,text=True,check=True);s=os.statvfs(R)
 return dict(du_kib={x.split()[1]:int(x.split()[0]) for x in r.stdout.splitlines()},available_bytes=s.f_bavail*s.f_frsize)
assert json.loads((D/'regression-terminal.json').read_text())['all_commands_exit_zero']
old=[];current=[]
for p in (R/'target/release/.fingerprint').glob('mycity-headless-*/lib-mycity_headless.json'):
 x=json.loads(p.read_text());key=p.parent.name.rsplit('-',1)[-1];files=list((R/'target/release/deps').glob('libmycity_headless-'+key+'*'));required=any(item[1]=='libproc' for item in x['deps'])
 for f in files:
  if not f.is_file() or f.suffix not in ['.rlib','.rmeta']:continue
  if required:current.append((f,metadata(f),sha(f)));continue
  s=f.stat();assert stat.S_ISREG(s.st_mode) and s.st_nlink==1
  old.append(dict(path=str(f.relative_to(R)),metadata=metadata(f),allocated_bytes=s.st_blocks*512,sha256=sha(f),graph=str(p.relative_to(R)),graph_sha256=sha(p),reason='obsolete macOS headless library graph lacks now mandatory libproc; pure regenerable rlib/rmeta cache, not an acceptance executable',retained='fixed old and new source/lock/exe + full test and actual measurement evidence remain'))
assert old and current
fixed=[(p,metadata(p),sha(p)) for p in (R/'work/candidate-binaries').rglob('*') if p.is_file()]
others={str(p.relative_to(R)):metadata(p) for p in (R/'target').rglob('*') if p.is_file() and str(p.relative_to(R)) not in {x['path'] for x in old}}
pre=no_users();before=disk();save('allowlist.json',old);save('before.json',dict(disk=before,occupancy=pre,protected_fixed_files=len(fixed),current_libraries=[str(p.relative_to(R)) for p,_,_ in current]))
for x in old:
 p=R/x['path'];assert metadata(p)==x['metadata'];p.unlink()
assert {str(p.relative_to(R)):metadata(p) for p in (R/'target').rglob('*') if p.is_file()}==others
for p,m,h in fixed+current:assert metadata(p)==m and sha(p)==h
post=no_users();after=disk();save('result.json',dict(deleted_files=len(old),allocated_bytes_removed=sum(x['allocated_bytes'] for x in old),before=before,after=after,filesystem_available_delta_bytes=after['available_bytes']-before['available_bytes'],attribution='inode allocation not exclusive APFS physical space; filesystem delta independently observed, not fully credited',other_target_metadata_unchanged=True,fixed_exe_and_snapshots_checked=len(fixed),current_libraries_checked=len(current),work_payload_deleted_bytes=0,post_occupancy=post));print((C/'result.json').read_text())
