#!/usr/bin/env python3
"""Archive immutable completed diagnostics, actual original worlds and source preimage."""
import hashlib,json,shutil,struct,zipfile
from pathlib import Path
D=Path(__file__).parent;R=D.resolve().parents[2]
for name,prefix in [('t13a2-store-borrowed-v1-02','borrowed-v1'),('t13a2-store-v2-03','v2')]:
 run=R/'work'/name
 for p,n in [(run/'model/report.json','model.json'),(run/'observer-report.json','observer.json')]:shutil.copyfile(p,D/f'{prefix}-{n}')
 with zipfile.ZipFile(D/f'{prefix}-measurements.zip','w',zipfile.ZIP_DEFLATED) as z:
  for n in ['host-samples.jsonl','model.stdout','model.stderr','model/events.jsonl']:z.write(run/n,n)
 manifest=[]
 with zipfile.ZipFile(D/f'{prefix}-world.zip','w',zipfile.ZIP_DEFLATED) as z:
  world=run/'model/world'
  for p in sorted(world.rglob('*')):
   if p.is_file():
    h=hashlib.sha256()
    with p.open('rb') as f:
     while b:=f.read(65536):h.update(b)
    n=str(p.relative_to(world));manifest.append({'path':n,'bytes':p.stat().st_size,'sha256':h.hexdigest()});z.write(p,n)
 (D/f'{prefix}-world-manifest.json').write_text(json.dumps(manifest,indent=2)+'\n')
shutil.copyfile(R/'work/t13a2-v2-reference/report.json',D/'reference.json')
with zipfile.ZipFile(D/'reference.zip','w',zipfile.ZIP_DEFLATED) as z:z.write(R/'work/t13a2-v2-reference/final-engine.json','final-engine.json')
names=['.cargo/config.toml','Cargo.toml','Cargo.lock','rust-toolchain.toml']
for folder in ['crates','fixtures']:names.extend(str(p.relative_to(R)) for p in (R/folder).rglob('*') if p.is_file())
h=hashlib.sha256()
with zipfile.ZipFile(D/'source.zip','w',zipfile.ZIP_DEFLATED) as z:
 for name in sorted(names):
  p=name.encode();b=(R/name).read_bytes();h.update(struct.pack('<Q',len(p)));h.update(p);h.update(struct.pack('<Q',len(b)));h.update(b);z.writestr(name,b)
(D/'source.json').write_text(json.dumps({'source_sha256':h.hexdigest(),'files':len(names)},indent=2)+'\n');print(h.hexdigest())
