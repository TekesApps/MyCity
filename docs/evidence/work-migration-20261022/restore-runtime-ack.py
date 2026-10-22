"""Preserve recovery-generated runtime ack, then restore the historical control-file bytes and metadata."""
import json,shutil,os,hashlib
from pathlib import Path
from migrate import R,D,T,B,save,sha,attrs
import xattrs
n=Path('r07-report-error-l1-01/observed/model/layout/r07-state.json');src=B/n;dst=T/n;E=D.parent
assert src.is_file() and dst.is_file() and dst.stat().st_nlink==1
before=src.read_bytes();after=dst.read_bytes();assert before!=after
(E/'historical-r07-state-before.json').write_bytes(before);(E/'recovery-r07-state-after.json').write_bytes(after)
save('recovery-runtime-ack-transition.json',dict(path=str(n),historical_sha256=hashlib.sha256(before).hexdigest(),recovery_sha256=hashlib.sha256(after).hexdigest(),reason='fixed source32c load_only refreshes runtime r07-state in input layout parent; no snapshot file changed; preserve both and restore historical control file before final full equality check',historical_evidence=str(E/'historical-r07-state-before.json'),new_recovery_evidence=str(E/'recovery-r07-state-after.json')))
shutil.copyfile(src,dst);os.chmod(dst,src.stat().st_mode&0o7777)
for k in xattrs.listxattr(dst):
 if k not in xattrs.listxattr(src):xattrs.removexattr(dst,k)
for k in xattrs.listxattr(src):xattrs.setxattr(dst,k,xattrs.getxattr(src,k))
os.utime(dst,ns=(dst.stat().st_atime_ns,src.stat().st_mtime_ns));assert sha(src)==sha(dst) and attrs(src)==attrs(dst)
os.utime(dst.parent,ns=(dst.parent.stat().st_atime_ns,src.parent.stat().st_mtime_ns))
save('recovery-runtime-ack-restored.json',dict(restored_historical_bytes_and_source_metadata=True,path=str(dst),current_device=dst.stat().st_dev,current_inode=dst.stat().st_ino,snapshot_inputs_modified=False))
print('Historical runtime ack restored; original and recovery-generated versions preserved.')
