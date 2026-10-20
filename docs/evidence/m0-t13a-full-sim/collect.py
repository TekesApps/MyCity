#!/usr/bin/env python3
"""Archive a completed diagnostic. Does not start, resume, or modify its world."""
import hashlib,json,shutil,struct,sys,zipfile
from pathlib import Path
D=Path(__file__).resolve().parent;R=D.parents[2]
run=R/'work'/sys.argv[1]
for src,dst in [('observer-report.json','observer.json'),('model/report.json','model.json'),('model/cpu-report.json','cpu.json')]:shutil.copyfile(run/src,D/dst)
with zipfile.ZipFile(D/'measurements.zip','w',zipfile.ZIP_DEFLATED) as z:
 for n in ['host-samples.jsonl','model.stdout','model.stderr','model/transcript.jsonl','model/cpu-targets.jsonl','model/cpu-final-engine.json']:z.write(run/n,n)
manifest=[]
with zipfile.ZipFile(D/'world.zip','w',zipfile.ZIP_DEFLATED) as z:
 for p in sorted((run/'model/world').rglob('*')):
  if p.is_file():
   n=p.relative_to(run/'model/world').as_posix();h=hashlib.sha256()
   with p.open('rb') as f:
    while b:=f.read(65536):h.update(b)
   manifest.append({'path':n,'bytes':p.stat().st_size,'sha256':h.hexdigest()});z.write(p,n)
(D/'world-manifest.json').write_text(json.dumps(manifest,indent=2)+'\n')
names=['.cargo/config.toml','Cargo.toml','Cargo.lock','rust-toolchain.toml']
for folder in ['crates','fixtures']:names.extend(p.relative_to(R).as_posix() for p in (R/folder).rglob('*') if p.is_file())
h=hashlib.sha256()
with zipfile.ZipFile(D/'source.zip','w',zipfile.ZIP_DEFLATED) as z:
 for name in sorted(names):
  p=name.encode();b=(R/name).read_bytes();h.update(struct.pack('<Q',len(p)));h.update(p);h.update(struct.pack('<Q',len(b)));h.update(b);z.writestr(name,b)
(D/'source.json').write_text(json.dumps({'source_sha256':h.hexdigest(),'files':len(names),'run':str(run)},indent=2)+'\n')
print(h.hexdigest())
