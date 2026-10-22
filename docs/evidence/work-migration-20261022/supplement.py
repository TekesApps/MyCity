"""Supplement copy identity checks; never modify historical same-inode receipts."""
import json,gzip,os,stat,subprocess,hashlib
from pathlib import Path
from migrate import R,D,S,T,B,save,sha,attrs
assert S.is_symlink() and S.resolve()==T and B.is_dir()
a=json.loads(gzip.decompress((D/'source-manifest.json.gz').read_bytes()));b=json.loads(gzip.decompress((D/'destination-manifest.json.gz').read_bytes()))
groups={}
for n,x in a.items():
 if 'sha256'in x:groups.setdefault((x['dev'],x['ino']),[]).append(n)
external=[]
for ns in groups.values():
 x=a[ns[0]]
 if x['nlink']>len(ns):
  external.append(dict(paths=ns,old_dev=x['dev'],old_inode=x['ino'],old_nlink=x['nlink'],links_outside_work=x['nlink']-len(ns),new_dev=b[ns[0]]['dev'],new_inode=b[ns[0]]['ino'],sha256=x['sha256'],old_same_inode_receipts='historical only; external original hardlinks remain untouched'))
save('external-hardlink-mapping.json',external)
flags=[]
for n in a:
 src=B/n;dst=T/n;x=src.lstat();y=dst.lstat()
 if x.st_flags!=y.st_flags:
  os.chflags(dst,x.st_flags,follow_symlinks=False);assert dst.lstat().st_flags==x.st_flags,n
  flags.append(dict(path=n,source_flags=x.st_flags,destination_before=y.st_flags,destination_after=dst.lstat().st_flags))
# work root is omitted from per-entry manifest, so verify it explicitly.
x=B.lstat();y=T.lstat();assert (x.st_mode,x.st_uid,x.st_gid)==(y.st_mode,y.st_uid,y.st_gid)
assert attrs(B)==attrs(T),'work root extended attributes differ'
os.chflags(T,x.st_flags,follow_symlinks=False);os.utime(T,ns=(y.st_atime_ns,x.st_mtime_ns));assert T.stat().st_mtime_ns==x.st_mtime_ns
save('root-and-flags.json',dict(root_mode=x.st_mode,uid=x.st_uid,gid=x.st_gid,xattrs=attrs(B),source_flags=x.st_flags,root_mtime_ns=x.st_mtime_ns,all_entry_flags_checked=len(a),flag_corrections=flags,acl_scope='rsync extended-attributes mode and access smoke tests; no claim of independent all-entry ACL equality'))
fixed=json.loads((R/'docs/evidence/core-m0-integration-109/fmt-corrected/fixed-executables.json').read_text());checks=[]
for row in fixed['executables']:
 p=Path(row['fixed']);assert p.is_file() and os.access(p,os.X_OK);assert sha(p)==row['sha256']
 checks.append(dict(path=str(p),resolved=str(p.resolve()),sha256=row['sha256'],executable=True))
save('external-fixed-executables-verified.json',checks)
save('supplement-result.json',dict(external_hardlink_groups=len(external),fixed_external_executables=len(checks),flags_all_entries_verified=True,root_metadata_checked=True,acl_full_independent_equality='not_claimed'))
print('Supplemental identity, external executable, root and BSD flag checks passed.')
