# Resume only after rsync copied all bytes; its second-resolution mtime caused the initial strict check to stop before switching.
from pathlib import Path
import json,os,subprocess
D=Path('/Users/frankqin/dev/MyCity/docs/evidence/target-migration-20261022')
s=(D/'migrate.py').read_text();exec(s[:s.index('assert S.is_dir()')])
assert S.is_dir() and not S.is_symlink() and T.is_dir() and not B.exists()
before=json.loads((D/'source-manifest.json').read_text());assert manifest(S)==before
copy=manifest(T);assert before.keys()==copy.keys()
for p,x in before.items():
 y=copy[p]
 for k in ['mode','sha256','link']:assert x.get(k)==y.get(k),(p,k)
 if 'sha256' in x:assert x['size']==y['size'],p
for p,x in sorted(before.items(),key=lambda v:-v[0].count('/')):
 os.utime(T/p,ns=((T/p).lstat().st_atime_ns,x['mtime_ns']),follow_symlinks=False)
copy=manifest(T)
for p,x in before.items():
 assert x['mtime_ns']==copy[p]['mtime_ns'],p
 if 'sha256' in x:assert x['sha256']==copy[p]['sha256'] and x['size']==copy[p]['size'],p
groups={}
for p,x in before.items():
 if 'sha256' in x:groups.setdefault((x['dev'],x['ino']),[]).append(p)
for ps in groups.values():assert len({(copy[p]['dev'],copy[p]['ino']) for p in ps})==1
save('destination-manifest.json',copy)
q=subprocess.run(['lsof','-nP','+D',str(S)],capture_output=True,text=True);save('occupancy-switch.json',dict(code=q.returncode,stdout=q.stdout,stderr=q.stderr));assert q.returncode==1 and not q.stdout and not q.stderr
assert manifest(S)==before
S.rename(B)
try:S.symlink_to(T,target_is_directory=True)
except:B.rename(S);raise
assert S.resolve()==T and manifest(S)==copy
save('switch.json',dict(verified=True,files=sum('sha256' in x for x in before.values()),source=str(S),destination=str(T),rollback_original=str(B),all_file_bytes_modes_mtimes_verified=True,internal_hardlinks_preserved=True,free_after_copy=free()))
save('initial-copy-check-failure.json',dict(reason='rsync truncates subsecond mtimes; byte copy passed but strict nanosecond check stopped before switch',remedy='restored exact source mtime_ns and reverified every byte, mode and internal hardlink group before switching'))
print('Verified migration and link switch; original retained for Cargo check.')
