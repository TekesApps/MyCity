import os,json,hashlib,subprocess,shutil,time
from pathlib import Path
R=Path('/Users/frankqin/dev/MyCity');D=R/'docs/evidence/target-migration-20261022';S=R/'target';T=Path('/Volumes/HP P900/dev/MyCity/target');B=R/'target-migration-original-20261022'
def save(n,v): (D/n).write_text(json.dumps(v,indent=2)+'\n')
def manifest(root):
 result={}
 for a,ds,fs in os.walk(root):
  for n in sorted(ds+fs):
   p=Path(a)/n;s=p.lstat();row=dict(mode=s.st_mode,size=s.st_size,mtime_ns=s.st_mtime_ns,blocks=s.st_blocks,dev=s.st_dev,ino=s.st_ino,nlink=s.st_nlink)
   if p.is_symlink():row['link']=os.readlink(p)
   elif p.is_file():
    h=hashlib.sha256()
    with p.open('rb') as f:
     for b in iter(lambda:f.read(1024*1024),b''):h.update(b)
    row['sha256']=h.hexdigest()
   result[str(p.relative_to(root))]=row
 return result
def free():return {str(p):shutil.disk_usage(p).free for p in [R,T.parent]}
assert S.is_dir() and not S.is_symlink() and not T.exists() and not B.exists()
q=subprocess.run(['lsof','-nP','+D',str(S)],capture_output=True,text=True);save('occupancy-before.json',dict(code=q.returncode,stdout=q.stdout,stderr=q.stderr));assert q.returncode==1 and not q.stdout and not q.stderr
T.parent.mkdir(parents=True,exist_ok=True);save('free-before.json',free());before=manifest(S);save('source-manifest.json',before)
q=subprocess.run(['/usr/bin/rsync','-aH',str(S)+'/',str(T)+'/'],capture_output=True,text=True);save('copy.json',dict(code=q.returncode,stdout=q.stdout,stderr=q.stderr));assert q.returncode==0
copy=manifest(T);assert before.keys()==copy.keys()
for p,x in before.items():
 y=copy[p]
 for k in ['mode','sha256','link']:
  assert x.get(k)==y.get(k),(p,k)
 if 'sha256' in x:assert x['size']==y['size'] and x['mtime_ns']==y['mtime_ns'],p
# All original internal hardlink groups must remain hardlinked at destination.
groups={}
for p,x in before.items():
 if 'sha256' in x:groups.setdefault((x['dev'],x['ino']),[]).append(p)
for ps in groups.values():assert len({(copy[p]['dev'],copy[p]['ino']) for p in ps})==1
save('destination-manifest.json',copy)
q=subprocess.run(['lsof','-nP','+D',str(S)],capture_output=True,text=True);save('occupancy-switch.json',dict(code=q.returncode,stdout=q.stdout,stderr=q.stderr));assert q.returncode==1 and not q.stdout and not q.stderr
assert manifest(S)==before
S.rename(B)
try:S.symlink_to(T,target_is_directory=True)
except: B.rename(S);raise
assert S.resolve()==T and manifest(S)==copy
save('switch.json',dict(verified=True,files=sum('sha256' in x for x in before.values()),source=str(S),destination=str(T),rollback_original=str(B),all_file_bytes_modes_mtimes_verified=True,internal_hardlinks_preserved=True,free_after_copy=free(),timestamp=time.time()))
print('Copied, byte-verified, symlink switched. Original retained pending Cargo verification.')
