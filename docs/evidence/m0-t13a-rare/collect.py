#!/usr/bin/env python3
"""Archive a terminal run. Does not drive/retry/change model state."""
import hashlib,json,struct,sys,zipfile,shutil
from pathlib import Path
D=Path(__file__).resolve().parent;R=D.parents[2]
for name in sys.argv[1:]:
 base=R/'work'/name;out=D/name;out.mkdir(exist_ok=True)
 manifest=[]
 with zipfile.ZipFile(out/'artifacts.zip','w',zipfile.ZIP_DEFLATED) as z:
  for p in sorted(base.rglob('*')):
   if p.is_file():
    n=p.relative_to(base).as_posix();b=p.read_bytes();manifest.append({'path':n,'bytes':len(b),'sha256':hashlib.sha256(b).hexdigest()});z.writestr(n,b)
 (out/'manifest.json').write_text(json.dumps(manifest,indent=2)+'\n')
 for p in [base/'observer-report.json',base/'model/report.json']:
  if p.exists():shutil.copyfile(p,out/p.name)
names=['.cargo/config.toml','Cargo.toml','Cargo.lock','rust-toolchain.toml']
for folder in ['crates','fixtures']:names.extend(p.relative_to(R).as_posix() for p in (R/folder).rglob('*') if p.is_file())
h=hashlib.sha256()
with zipfile.ZipFile(D/'source.zip','w',zipfile.ZIP_DEFLATED) as z:
 for name in sorted(names):
  p=name.encode();b=(R/name).read_bytes();h.update(struct.pack('<Q',len(p)));h.update(p);h.update(struct.pack('<Q',len(b)));h.update(b);z.writestr(name,b)
(D/'source.json').write_text(json.dumps({'source_sha256':h.hexdigest(),'files':len(names),'binary_sha256':hashlib.sha256((R/'target/release/mycity-headless').read_bytes()).hexdigest(),'rust':'1.85.1'},indent=2)+'\n')
print(h.hexdigest())
