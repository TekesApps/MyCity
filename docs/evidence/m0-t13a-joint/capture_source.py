import hashlib,json,struct,sys,zipfile
from pathlib import Path
D=Path(__file__).resolve().parent;R=D.parents[2];prefix=sys.argv[1];names=['.cargo/config.toml','Cargo.toml','Cargo.lock','rust-toolchain.toml']
for directory in ['crates','fixtures']:names.extend(p.relative_to(R).as_posix() for p in (R/directory).rglob('*') if p.is_file())
h=hashlib.sha256()
with zipfile.ZipFile(D/(prefix+'.zip'),'x',zipfile.ZIP_DEFLATED) as z:
 for name in sorted(names):
  n=name.encode();b=(R/name).read_bytes();h.update(struct.pack('<Q',len(n)));h.update(n);h.update(struct.pack('<Q',len(b)));h.update(b);z.writestr(name,b)
(D/(prefix+'.json')).write_text(json.dumps({'source_sha256':h.hexdigest(),'binary_sha256':hashlib.sha256((R/'target/release/mycity-headless').read_bytes()).hexdigest(),'files':len(names),'rust':'1.85.1','support_archive':'source-support.zip'},indent=2)+'\n')
print(h.hexdigest())
