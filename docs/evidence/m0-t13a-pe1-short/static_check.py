#!/usr/bin/env python3
import hashlib,json,struct,zipfile
from pathlib import Path
D=Path(__file__).resolve().parent;R=D.parents[2]
sha=lambda b:hashlib.sha256(b).hexdigest()
s=json.loads((D/'source.json').read_text());h=hashlib.sha256()
with zipfile.ZipFile(D/'source.zip') as z:
 for n in sorted(z.namelist()):
  p=n.encode();b=z.read(n);h.update(struct.pack('<Q',len(p)));h.update(p);h.update(struct.pack('<Q',len(b)));h.update(b)
  assert b==(R/n).read_bytes(),n
 assert sha(z.read('fixtures/m0-t13a-acceptance.json'))=='876ea995ea7834bfcea59b2ef8fa56d502aed9006477bcc67bcaa3f5cd944508'
assert h.hexdigest()==s['source_sha256'] and sha((R/'target/release/mycity-headless').read_bytes())==s['binary_sha256']
p=json.loads((D/'reference-provenance.json').read_text());assert sha((R/p['archive']).read_bytes())==p['archive_sha256']
with zipfile.ZipFile(R/p['archive']) as z:
 v=json.loads(z.read(p['member']))['economy_probe']['pe1'];expected={k:v[k]['world'] for k in ['debt_exit','prepaid_cancel','refund_shortfall','suspended_shift']}
assert expected==json.loads((D/'reference-worlds.json').read_text()) and sha((D/'reference-worlds.json').read_bytes())==p['reference_sha256']
for folder in D.glob('t13a2g-*'):
 manifest=json.loads((folder/'manifest.json').read_text())
 with zipfile.ZipFile(folder/'artifacts.zip') as z:
  assert set(z.namelist())=={f['path'] for f in manifest}
  for f in manifest:
   b=z.read(f['path']);assert len(b)==f['bytes'] and sha(b)==f['sha256']
print(json.dumps({'source_sha256':s['source_sha256'],'binary_sha256':s['binary_sha256'],'frozen_acceptance_unchanged':True,'historical_reference_verified':True,'outer_archive_hashes_verified':True},indent=2))
