#!/usr/bin/env python3
import hashlib,json,struct,zipfile
from pathlib import Path
D=Path(__file__).resolve().parent;R=D.parents[2];info=json.loads((D/'source.json').read_text());sha=lambda b:hashlib.sha256(b).hexdigest();h=hashlib.sha256()
with zipfile.ZipFile(D/'source.zip') as z:
 assert len(z.namelist())==info['source_files']
 for n in sorted(z.namelist()):
  p=n.encode();b=z.read(n);assert b==(R/n).read_bytes(),n;h.update(struct.pack('<Q',len(p)));h.update(p);h.update(struct.pack('<Q',len(b)));h.update(b)
assert h.hexdigest()==info['source_sha256']
assert sha((R/'target/release/mycity-headless').read_bytes())==info['binary_sha256']
assert sha((R/'fixtures/m0-t13a-acceptance.json').read_bytes())==info['frozen_acceptance_sha256']=='876ea995ea7834bfcea59b2ef8fa56d502aed9006477bcc67bcaa3f5cd944508'
for name,key in [('runner.py','runner_sha256'),('observe.py','observer_sha256'),('source-support.zip','source_support_sha256')]:assert sha((D/name).read_bytes())==info[key]
for p in D.glob('t13a49-*/manifest.json'):
 rows=json.loads(p.read_text())
 with zipfile.ZipFile(p.parent/'artifacts.zip') as z:
  assert set(z.namelist())=={r['path'] for r in rows}
  for r in rows:
   b=z.read(r['path']);assert len(b)==r['bytes'] and sha(b)==r['sha256']
print(json.dumps({'source_sha256':info['source_sha256'],'files':info['source_files'],'frozen_load_unchanged':True,'binary_and_scripts_unchanged':True,'scope':'static integrity only; no runtime/performance/8h acceptance'},indent=2))
