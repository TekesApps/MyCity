#!/usr/bin/env python3
import hashlib,json,struct,zipfile
from pathlib import Path
D=Path(__file__).resolve().parent;R=D.parents[2];sha=lambda b:hashlib.sha256(b).hexdigest()
names=['.cargo/config.toml','Cargo.toml','Cargo.lock','rust-toolchain.toml']
for folder in ['crates','fixtures']:names.extend(p.relative_to(R).as_posix() for p in (R/folder).rglob('*') if p.is_file())
h=hashlib.sha256()
with zipfile.ZipFile(D/'source.zip','x',zipfile.ZIP_DEFLATED) as z:
 for n in sorted(names):
  b=(R/n).read_bytes();p=n.encode();h.update(struct.pack('<Q',len(p)));h.update(p);h.update(struct.pack('<Q',len(b)));h.update(b);z.writestr(n,b)
support=R/'docs/evidence/m0-t13a-pe1-short/reference-worlds.json'
with zipfile.ZipFile(D/'source-support.zip','x',zipfile.ZIP_DEFLATED) as z:z.write(support,support.relative_to(R).as_posix())
info={'source_sha256':h.hexdigest(),'source_files':len(names),'binary_sha256':sha((R/'target/release/mycity-headless').read_bytes()),'frozen_acceptance_sha256':sha((R/'fixtures/m0-t13a-acceptance.json').read_bytes()),'observer_sha256':sha((D/'observe.py').read_bytes()),'source_support_sha256':sha((D/'source-support.zip').read_bytes()),'rust':'1.85.1'}
assert info['frozen_acceptance_sha256']=='876ea995ea7834bfcea59b2ef8fa56d502aed9006477bcc67bcaa3f5cd944508'
(D/'source.json').write_text(json.dumps(info,indent=2)+'\n');print(json.dumps(info,indent=2))
