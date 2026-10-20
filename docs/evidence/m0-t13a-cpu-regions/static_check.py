#!/usr/bin/env python3
"""Release links, unchanged measured Rust/fixtures, and frozen acceptance audit."""
import hashlib,json,re,struct,zipfile,urllib.parse
from pathlib import Path
D=Path(__file__).resolve().parent;R=D.parents[2];n=0
paths=['README.md','docs/m0-model-scale-performance.md','docs/m0-implementation-status.md','docs/m0-implementation-plan.md','docs/evidence/m0-t13a-cpu-regions/STATUS.md','docs/evidence/m0-t13a-cpu-regions/advisor-review.md']
for name in paths:
 p=R/name
 for target in re.findall(r'\]\(([^)]+)\)',p.read_text()):
  if '://' in target or target.startswith('#'):continue
  target=urllib.parse.unquote(target.split('#')[0].strip('<>'));assert (p.parent/target).exists(),(name,target);n+=1
assert '仓库发布版本：`v0.5.37`' in (R/'README.md').read_text()
names=['.cargo/config.toml','Cargo.toml','Cargo.lock','rust-toolchain.toml']
for folder in ['crates','fixtures']:names.extend(p.relative_to(R).as_posix() for p in (R/folder).rglob('*') if p.is_file())
h=hashlib.sha256()
with zipfile.ZipFile(D/'source.zip') as z:
 assert sorted(names)==sorted(z.namelist())
 for name in sorted(names):
  b=(R/name).read_bytes();assert b==z.read(name);p=name.encode();h.update(struct.pack('<Q',len(p)));h.update(p);h.update(struct.pack('<Q',len(b)));h.update(b)
assert h.hexdigest()==json.loads((D/'source.json').read_text())['source_sha256']
assert hashlib.sha256((R/'fixtures/m0-t13a-acceptance.json').read_bytes()).hexdigest()=='876ea995ea7834bfcea59b2ef8fa56d502aed9006477bcc67bcaa3f5cd944508'
print(json.dumps({'release':'v0.5.37','local_links':n,'source_sha256':h.hexdigest(),'measured_source_unchanged':True,'frozen_acceptance_unchanged':True,'formal_performance_gate':'not_accepted_by_static_checks'},indent=2))
