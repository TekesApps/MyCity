#!/usr/bin/env python3
"""Archive a completed actual storage diagnostic; no mutation of the world."""
import hashlib,json,shutil,struct,zipfile
from pathlib import Path
D=Path(__file__).parent;R=D.resolve().parents[2];run=R/'work/t13a2-store-baseline-01';world=run/'model/world'
sha=lambda b:hashlib.sha256(b).hexdigest()
for src,n in [(run/'model/report.json','baseline.json'),(run/'observer-report.json','observer.json'),(R/'work/t13a2-startup-refactor-reference-output.json','refactor-reference.json')]:shutil.copyfile(src,D/n)
files=[]
with zipfile.ZipFile(D/'world.zip','w',zipfile.ZIP_DEFLATED) as z:
 for p in sorted(world.rglob('*')):
  if p.is_file():
   n=str(p.relative_to(world));h=hashlib.sha256()
   with p.open('rb') as f:
    while b:=f.read(65536):h.update(b)
   files.append({'path':n,'bytes':p.stat().st_size,'sha256':h.hexdigest()});z.write(p,n)
(D/'world-manifest.json').write_text(json.dumps({'files':files,'all_original_receipts_preserved':True,'source_world_not_changed_or_removed':True},indent=2)+'\n')
with zipfile.ZipFile(D/'measurements.zip','w',zipfile.ZIP_DEFLATED) as z:
 for n in ['host-samples.jsonl','model.stdout','model.stderr','model/events.jsonl']:z.write(run/n,n)
names=['.cargo/config.toml','Cargo.toml','Cargo.lock','rust-toolchain.toml']
for folder in ['crates','fixtures']:names.extend(str(p.relative_to(R)) for p in (R/folder).rglob('*') if p.is_file())
h=hashlib.sha256()
with zipfile.ZipFile(D/'source.zip','w',zipfile.ZIP_DEFLATED) as z:
 for n in sorted(names):
  b=(R/n).read_bytes();p=n.encode();h.update(struct.pack('<Q',len(p)));h.update(p);h.update(struct.pack('<Q',len(b)));h.update(b);z.writestr(n,b)
(D/'source.json').write_text(json.dumps({'source_sha256':h.hexdigest(),'source_files':len(names),'zip_sha256':sha((D/'source.zip').read_bytes())},indent=2)+'\n')
print(h.hexdigest())
