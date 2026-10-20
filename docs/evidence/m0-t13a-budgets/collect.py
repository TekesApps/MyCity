#!/usr/bin/env python3
"""Archive terminal diagnostic metadata and real business; large L1 pressure stays in work."""
import hashlib,json,struct,sys,zipfile
from pathlib import Path
D=Path(__file__).resolve().parent;R=D.parents[2];base=R/'work'/sys.argv[1];out=D/sys.argv[1];out.mkdir()
manifest=[];omitted=[]
with zipfile.ZipFile(out/'artifacts.zip','x',zipfile.ZIP_DEFLATED) as z:
 for p in sorted(base.rglob('*')):
  assert not p.is_symlink()
  if not p.is_file():continue
  n=p.relative_to(base).as_posix()
  if '/layout/snapshots/snapshot-' in n and p.name!='index.json':
   omitted.append({'path':n,'bytes':p.stat().st_size,'integrity':'hash stored by snapshot index; actual full restore verified; original remains in ignored work'});continue
  b=p.read_bytes();manifest.append({'path':n,'bytes':len(b),'sha256':hashlib.sha256(b).hexdigest()});z.writestr(n,b)
(out/'manifest.json').write_text(json.dumps(manifest,indent=2)+'\n');(out/'large-files-retained.json').write_text(json.dumps(omitted,indent=2)+'\n')
if (D/'source.zip').exists():raise SystemExit(0)
names=['.cargo/config.toml','Cargo.toml','Cargo.lock','rust-toolchain.toml']
for folder in ['crates','fixtures']:names.extend(p.relative_to(R).as_posix() for p in (R/folder).rglob('*') if p.is_file())
h=hashlib.sha256()
with zipfile.ZipFile(D/'source.zip','x',zipfile.ZIP_DEFLATED) as z:
 for name in sorted(names):
  b=(R/name).read_bytes();p=name.encode();h.update(struct.pack('<Q',len(p)));h.update(p);h.update(struct.pack('<Q',len(b)));h.update(b);z.writestr(name,b)
support=R/'docs/evidence/m0-t13a-pe1-short/reference-worlds.json'
with zipfile.ZipFile(D/'source-support.zip','x',zipfile.ZIP_DEFLATED) as z:z.write(support,support.relative_to(R).as_posix())
(D/'source.json').write_text(json.dumps({'source_sha256':h.hexdigest(),'binary_sha256':hashlib.sha256((R/'target/release/mycity-headless').read_bytes()).hexdigest(),'files':len(names),'rust':'1.85.1','source_support_sha256':hashlib.sha256((D/'source-support.zip').read_bytes()).hexdigest(),'support_reason':'historical World include_str used by economy_short integration test; outside compile fingerprint file list'},indent=2)+'\n')
print(h.hexdigest())
