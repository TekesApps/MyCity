#!/usr/bin/env python3
"""Archive only terminal runs. No formal-performance classification is inferred."""
import hashlib,json,struct,zipfile
from pathlib import Path
D=Path(__file__).resolve().parent;R=D.parents[2]
sha=lambda b:hashlib.sha256(b).hexdigest()
for work,folder in [('t13a3e-model-tasks-functional-02','second-wire-pass'),('t13a3e-model-tasks-functional-03','final-functional'),('t13a3e-model-tasks-validation','validation')]:
 out=D/folder;out.mkdir();rows=[]
 with zipfile.ZipFile(out/'artifacts.zip','x',zipfile.ZIP_DEFLATED) as z:
  for p in sorted((R/'work'/work).rglob('*')):
   assert not p.is_symlink()
   if not p.is_file():continue
   b=p.read_bytes();name=p.relative_to(R/'work'/work).as_posix();z.writestr(name,b);rows.append({'path':name,'bytes':len(b),'sha256':sha(b)})
 (out/'manifest.json').write_text(json.dumps(rows,indent=2)+'\n')
names=['.cargo/config.toml','Cargo.toml','Cargo.lock','rust-toolchain.toml']
for base in ['crates','fixtures']:names += [p.relative_to(R).as_posix() for p in (R/base).rglob('*') if p.is_file()]
h=hashlib.sha256()
with zipfile.ZipFile(D/'source.zip','x',zipfile.ZIP_DEFLATED) as z:
 for name in sorted(names):
  b=(R/name).read_bytes();n=name.encode();h.update(struct.pack('<Q',len(n)));h.update(n);h.update(struct.pack('<Q',len(b)));h.update(b);z.writestr(name,b)
support=R/'docs/evidence/m0-t13a-pe1-short/reference-worlds.json'
with zipfile.ZipFile(D/'source-support.zip','x',zipfile.ZIP_DEFLATED) as z:z.write(support,support.relative_to(R).as_posix())
info={'source_sha256':h.hexdigest(),'binary_sha256':sha((R/'target/release/mycity-headless').read_bytes()),'source_files':len(names),'source_support_sha256':sha((D/'source-support.zip').read_bytes()),'functional_harness_sha256':sha((D/'functional.py').read_bytes()),'rust':'1.85.1','scope':'final source focused regressions and independent full CPU comparison; no final-source full workspace rerun or full Sim performance gate'}
(D/'source.json').write_text(json.dumps(info,indent=2)+'\n');print(json.dumps(info,indent=2))
