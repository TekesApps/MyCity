"""Remove only the fully verified rollback work directory after actual restore and advisor release."""
import json,gzip,stat,shutil,subprocess,hashlib
from pathlib import Path
from migrate import R,D,S,T,B,save,sha,manifest,occupied,free
E=D.parent
assert S.is_symlink() and S.resolve()==T and B.is_dir() and not B.is_symlink()
assert json.loads((D/'supplement-result.json').read_text())['fixed_external_executables']==10
checks=json.loads((E/'recovery-results.json').read_text());assert len(checks)==3 and all(x['exit']==0 for x in checks)
assert (E/'advisor-release.md').is_file()
a=json.loads(gzip.decompress((D/'source-manifest.json.gz').read_bytes()));dest=json.loads(gzip.decompress((D/'destination-manifest.json.gz').read_bytes()));assert manifest(B)==a,'rollback contents or metadata changed'
# Check every migrated regular file byte, link, mode, owner, timestamp and xattr again. New restore output is outside the original allowlist.
from migrate import attrs
for n,x in a.items():
 p=T/n;s=p.lstat()
 assert (s.st_mode,s.st_uid,s.st_gid,s.st_mtime_ns)==(x['mode'],x['uid'],x['gid'],x['mtime_ns']),n
 assert attrs(p)==dest[n]['xattrs'],n
 if 'sha256'in x:assert s.st_size==x['size'] and sha(p)==x['sha256'],n
 if 'link'in x:assert p.is_symlink() and str(p.readlink())==x['link'],n
 if x.get('kind')=='inactive-unix-socket':assert stat.S_ISSOCK(s.st_mode),n
for x in json.loads((D/'protected-after.json').read_text()):assert sha(R/x['path'])==x['sha256']
for x in json.loads((D/'external-fixed-executables-verified.json').read_text()):assert sha(Path(x['path']))==x['sha256']
occupied('occupancy-retire-original',B);occupied('occupancy-retire-destination',T)
groups={}
for n,x in a.items():
 if 'sha256'in x:groups.setdefault((x['dev'],x['ino']),[]).append(n)
alloc=sum(a[ns[0]]['blocks']*512 for ns in groups.values());released=sum(a[ns[0]]['blocks']*512 for ns in groups.values() if a[ns[0]]['nlink']==len(ns));before=free()
save('retirement-intent.json',dict(exact_root=str(B),exact_original_manifest='source-manifest.json.gz',unique_regular_file_allocation_bytes=alloc,allocation_bytes_no_external_links=released,source_and_destination_fully_reverified=True,recovery_passed=3,protected_originals_preserved=190,free_before=before))
shutil.rmtree(B)
assert not B.exists() and S.is_symlink() and S.resolve()==T
for x in json.loads((D/'protected-after.json').read_text()):assert sha(R/x['path'])==x['sha256']
after=free();save('retirement-result.json',dict(original_removed=True,source_alias=str(S),destination=str(T),unique_regular_file_allocation_bytes=alloc,allocation_bytes_no_external_links=released,allocation_with_external_links_retained=alloc-released,free_before=before,free_after=after,observational_available_delta={k:after[k]-before[k] for k in before},physical_exclusive_APFS_reclaim='not_claimed',protected_originals_sha_rechecked_after=190,restorable_originals='preserved in full on external SSD; historical already-retired snapshots remain not directly restorable'))
print((D/'retirement-result.json').read_text())
