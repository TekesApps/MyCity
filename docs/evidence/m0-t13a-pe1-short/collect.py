#!/usr/bin/env python3
import hashlib,json,struct,zipfile
from pathlib import Path
D=Path(__file__).resolve().parent;R=D.parents[2]
for ident in ['t13a2g-pilot-01','t13a2g-pilot-02','t13a2g-full-01']:
 out=D/ident;out.mkdir(exist_ok=True);base=R/'work'/ident
 files={}
 if base.exists():
  for p in base.rglob('*'):
   if p.is_file():files[p.relative_to(base).as_posix()]=p
 for name in ['observer-report.json','host-samples.jsonl','model.stdout','model.stderr']:
  if (out/name).exists():files['observer/'+name]=out/name
 manifest=[]
 with zipfile.ZipFile(out/'artifacts.zip','x',zipfile.ZIP_DEFLATED) as z:
  for name,p in sorted(files.items()):
   b=p.read_bytes();manifest.append({'path':name,'bytes':len(b),'sha256':hashlib.sha256(b).hexdigest()});z.writestr(name,b)
 (out/'manifest.json').write_text(json.dumps(manifest,indent=2)+'\n')
names=['.cargo/config.toml','Cargo.toml','Cargo.lock','rust-toolchain.toml']
for folder in ['crates','fixtures']:names.extend(p.relative_to(R).as_posix() for p in (R/folder).rglob('*') if p.is_file())
h=hashlib.sha256()
with zipfile.ZipFile(D/'source.zip','x',zipfile.ZIP_DEFLATED) as z:
 for name in sorted(names):
  b=(R/name).read_bytes();p=name.encode();h.update(struct.pack('<Q',len(p)));h.update(p);h.update(struct.pack('<Q',len(b)));h.update(b);z.writestr(name,b)
(D/'source.json').write_text(json.dumps({'source_sha256':h.hexdigest(),'binary_sha256':hashlib.sha256((R/'target/release/mycity-headless').read_bytes()).hexdigest(),'files':len(names),'rust':'1.85.1'},indent=2)+'\n')
print(h.hexdigest())
