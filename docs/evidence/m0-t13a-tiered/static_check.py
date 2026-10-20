#!/usr/bin/env python3
"""Check release navigation, exact compiled preimage, and original acceptance."""
import hashlib,json,re,struct,urllib.parse,zipfile
from pathlib import Path
D=Path(__file__).resolve().parent;R=D.parents[2]
paths=['README.md','docs/m0-model-scale-performance.md','docs/m0-implementation-plan.md','docs/m0-implementation-status.md','docs/evidence/m0-t13a-tiered/STATUS.md','docs/evidence/m0-t13a-tiered/advisor-review.md']
n=0
for name in paths:
 p=R/name
 for target in re.findall(r'\]\(([^)]+)\)',p.read_text()):
  if '://' in target or target.startswith('#'):continue
  target=urllib.parse.unquote(target.split('#')[0].strip('<>'))
  assert (p.parent/target).exists(),(name,target)
  n+=1
assert '仓库发布版本：`v0.5.36`' in (R/'README.md').read_text()
assert hashlib.sha256((R/'fixtures/m0-t13a-acceptance.json').read_bytes()).hexdigest()=='876ea995ea7834bfcea59b2ef8fa56d502aed9006477bcc67bcaa3f5cd944508'
current=['.cargo/config.toml','Cargo.toml','Cargo.lock','rust-toolchain.toml']
for folder in ['crates','fixtures']:current.extend(p.relative_to(R).as_posix() for p in (R/folder).rglob('*') if p.is_file())
h=hashlib.sha256()
with zipfile.ZipFile(D/'source.zip') as z:
 assert sorted(current)==sorted(z.namelist())
 for name in sorted(z.namelist()):
  assert (R/name).read_bytes()==z.read(name),name
  p=name.encode();b=z.read(name);h.update(struct.pack('<Q',len(p)));h.update(p);h.update(struct.pack('<Q',len(b)));h.update(b)
assert h.hexdigest()==json.loads((D/'source.json').read_text())['source_sha256']
print(json.dumps({'local_links_checked':n,'release':'v0.5.36','all_compiled_source_preimage_unchanged':True,'frozen_acceptance_unchanged':True,'memory_and_performance_gates':'not_passed_by_static_checks'},indent=2))
