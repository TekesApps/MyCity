"""One-shot full-byte work relocation. Stop before removing the original; verify recovery separately."""
import os,json,gzip,hashlib,subprocess,shutil,time,stat
import xattrs
from pathlib import Path
R=Path('/Users/frankqin/dev/MyCity');D=R/'docs/evidence/work-migration-20261022/attempt02';S=R/'work';T=Path('/Volumes/HP P900/dev/MyCity/work');B=R/'work-migration-original-20261022'
def save(n,v):
 b=(json.dumps(v,ensure_ascii=False,indent=2)+'\n').encode()
 if n.endswith('.gz'):b=gzip.compress(b,mtime=0)
 with (D/n).open('xb') as f:f.write(b)
def sha(p):
 h=hashlib.sha256()
 with p.open('rb') as f:
  for b in iter(lambda:f.read(4*1024**2),b''):h.update(b)
 return h.hexdigest()
def attrs(p):return {n:hashlib.sha256(xattrs.getxattr(p,n,follow_symlinks=False)).hexdigest() for n in xattrs.listxattr(p,follow_symlinks=False)}
def manifest(root):
 rows={}
 for a,ds,fs in os.walk(root,followlinks=False):
  for n in sorted(ds+fs):
   p=Path(a)/n;s=p.lstat();r=dict(mode=s.st_mode,uid=s.st_uid,gid=s.st_gid,size=s.st_size,mtime_ns=s.st_mtime_ns,dev=s.st_dev,ino=s.st_ino,blocks=s.st_blocks,nlink=s.st_nlink,xattrs=attrs(p))
   if p.is_symlink():r['link']=os.readlink(p)
   elif p.is_file():r['sha256']=sha(p)
   else:assert stat.S_ISDIR(s.st_mode),('unsupported special file',str(p))
   rows[str(p.relative_to(root))]=r
 return rows
def occupied(label,p):
 q=subprocess.run(['lsof','-nP','+D',str(p)],capture_output=True,text=True);save(label+'.json',dict(code=q.returncode,stdout=q.stdout,stderr=q.stderr));assert q.returncode==1 and not q.stdout and not q.stderr

def free():return {str(p):shutil.disk_usage(p).free for p in [R,T.parent]}
def wip():
 out={}
 for n in subprocess.check_output(['git','ls-files','--cached'],cwd=R,text=True).splitlines():
  p=R/n
  if p.is_file():out[n]=sha(p)
 return out
if __name__=='__main__':
 assert S.is_dir() and not S.is_symlink() and not T.exists() and not B.exists()
 occupied('occupancy-before',S);save('free-before.json',free());save('tracked-files-before.json',wip())
 subprocess.run(['ps','-axo','pid,ppid,etime,command'],stdout=(D/'processes-before.txt').open('x'),check=True)
 before=manifest(S);save('source-manifest.json.gz',before);print('Source full SHA manifest complete',len(before),flush=True)
 protected=json.loads((R/'docs/evidence/disk-reclamation-focused-20261022/protected-after.json').read_text());assert len(protected)==190
 for x in protected:
  n=str(Path(x['path']).relative_to('work'));assert before[n]['sha256']==x['sha256'],x['path']
 save('protected-before.json',protected)
 q=subprocess.run(['/usr/bin/rsync','-aH','--extended-attributes',str(S)+'/',str(T)+'/'],capture_output=True,text=True);save('copy.json',dict(code=q.returncode,stdout=q.stdout,stderr=q.stderr));assert q.returncode==0
 print('Copy complete; verifying every destination byte',flush=True)
 # Restore exact nanosecond timestamps and xattrs that macOS openrsync may omit.
 for n,x in sorted(before.items(),key=lambda v:-v[0].count('/')):
  p=T/n
  for key in xattrs.listxattr(p,follow_symlinks=False):
   if key not in x['xattrs']:xattrs.removexattr(p,key,follow_symlinks=False)
  for key in x['xattrs']:xattrs.setxattr(p,key,xattrs.getxattr(S/n,key,follow_symlinks=False),follow_symlinks=False)
  os.utime(p,ns=(p.lstat().st_atime_ns,x['mtime_ns']),follow_symlinks=False)
 after=manifest(T);assert before.keys()==after.keys()
 for n,x in before.items():
  y=after[n]
  for k in ['mode','uid','gid','mtime_ns','sha256','link','xattrs']:assert x.get(k)==y.get(k),(n,k)
  if 'sha256' in x:assert x['size']==y['size'],n
 groups={}
 for n,x in before.items():
  if 'sha256' in x:groups.setdefault((x['dev'],x['ino']),[]).append(n)
 for ns in groups.values():assert len({(after[n]['dev'],after[n]['ino']) for n in ns})==1
 # Recheck original identity and every digest before switching.
 assert manifest(S)==before,'source changed while copying'
 occupied('occupancy-switch',S)
 save('destination-manifest.json.gz',after)
 protected_after=[]
 for x in protected:
  n=str(Path(x['path']).relative_to('work'));y=after[n]
  assert y['sha256']==x['sha256']
  protected_after.append(dict(path=x['path'],metadata=[y['dev'],y['ino'],y['mode'],y['size'],y['mtime_ns'],y['blocks'],y['nlink']],sha256=x['sha256']))
 save('protected-after.json',protected_after)
 S.rename(B)
 try:S.symlink_to(T,target_is_directory=True)
 except:B.rename(S);raise
 assert S.is_symlink() and S.resolve()==T
 assert wip()==json.loads((D/'tracked-files-before.json').read_text())
 save('switch.json',dict(files=sum('sha256'in x for x in before.values()),entries=len(before),source=str(S),destination=str(T),rollback_original=str(B),protected_files_full_bytes_verified=190,all_file_bytes_verified=True,internal_hardlink_groups_verified=True,symlinks_xattrs_modes_owners_mtimes_verified=True,original_retained_pending_recovery=True,performance_storage='external_SSD_HP_P900',time_ns=time.time_ns(),free_after_copy=free()))
 print('Full verification passed; symlink switched. Original retained pending actual recovery.',flush=True)
