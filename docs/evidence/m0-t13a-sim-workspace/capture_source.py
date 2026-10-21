import hashlib,json,struct,sys,zipfile
from pathlib import Path
R=Path('/Users/frankqin/dev/MyCity');D=Path(__file__).resolve().parent;out=D/sys.argv[1];out.mkdir()
names=['.cargo/config.toml','Cargo.toml','Cargo.lock','rust-toolchain.toml']
for folder in ['crates','fixtures']:names.extend(p.relative_to(R).as_posix() for p in (R/folder).rglob('*') if p.is_file())
h=hashlib.sha256()
with zipfile.ZipFile(out/'source.zip','x',zipfile.ZIP_DEFLATED) as z:
 for n in sorted(names):
  b=(R/n).read_bytes();p=n.encode();h.update(struct.pack('<Q',len(p)));h.update(p);h.update(struct.pack('<Q',len(b)));h.update(b);z.writestr(n,b)
(out/'source.json').write_text(json.dumps({'source_sha256':h.hexdigest(),'files':len(names)},indent=2)+'\n')
print(h.hexdigest())
