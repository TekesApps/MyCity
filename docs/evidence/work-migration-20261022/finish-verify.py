"""Resume after macOS rsync's synthetic AppleDouble/xattr copy errors; do not overwrite first failure receipt."""
from pathlib import Path
import json,gzip,os,stat,socket,subprocess,hashlib
from migrate import R,D,S,T,B,save,manifest,occupied,attrs,sha,wip,free
import xattrs
assert S.is_dir() and not S.is_symlink() and T.is_dir() and not B.exists()
before=json.loads(gzip.decompress((D/'source-manifest.json.gz').read_bytes()));protected=json.loads((D/'protected-before.json').read_text());assert json.loads((D/'copy.json').read_text())['code']==23
after=manifest(T);assert before.keys()==after.keys()
for n,x in before.items():
 y=after[n]
 for k in ['mode','uid','gid','mtime_ns','sha256','link','xattrs','kind']:
  if k=='xattrs' and n=='t13a1b-layout-full-04/model/snapshots/.DS_Store':
   assert all(y[k].get(a)==v for a,v in x[k].items()) and set(y[k])-set(x[k])=={'com.apple.provenance'},(n,k)
  else:assert x.get(k)==y.get(k),(n,k)
 if 'sha256'in x:assert x['size']==y['size'],n
groups={}
for n,x in before.items():
 if 'sha256'in x:groups.setdefault((x['dev'],x['ino']),[]).append(n)
for ns in groups.values():assert len({(after[n]['dev'],after[n]['ino']) for n in ns})==1
print('Destination hashes match; rechecking source before switching',flush=True)
assert manifest(S)==before
occupied('occupancy-switch',S);save('destination-manifest.json.gz',after);rows=[]
for x in protected:
 n=str(Path(x['path']).relative_to('work'));y=after[n];assert y['sha256']==x['sha256'];rows.append(dict(path=x['path'],metadata=[y['dev'],y['ino'],y['mode'],y['size'],y['mtime_ns'],y['blocks'],y['nlink']],sha256=x['sha256']))
save('protected-after.json',rows)
S.rename(B)
try:S.symlink_to(T,target_is_directory=True)
except:B.rename(S);raise
assert S.is_symlink() and S.resolve()==T and wip()==json.loads((D/'tracked-files-before.json').read_text())
save('switch.json',dict(files=sum('sha256'in x for x in before.values()),entries=len(before),source=str(S),destination=str(T),rollback_original=str(B),protected_files_full_bytes_verified=190,all_file_bytes_verified=True,internal_hardlink_groups_verified=True,symlinks_xattrs_modes_owners_mtimes_verified=True,original_retained_pending_recovery=True,performance_storage='external_SSD_HP_P900',free_after_copy=free(),copy_implementation='resume-copy.py; original failed copy receipt retained'))
print('All source/destination bytes and tracked files match; link switched. Original retained for recovery.',flush=True)
