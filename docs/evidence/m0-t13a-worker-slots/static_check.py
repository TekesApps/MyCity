#!/usr/bin/env python3
import hashlib,json,struct,zipfile
from pathlib import Path
D=Path(__file__).resolve().parent;R=D.parents[2];sha=lambda b:hashlib.sha256(b).hexdigest();info=json.loads((D/'source.json').read_text());h=hashlib.sha256()
with zipfile.ZipFile(D/'source.zip') as z:
 for name in sorted(z.namelist()):
  b=z.read(name);n=name.encode();h.update(struct.pack('<Q',len(n)));h.update(n);h.update(struct.pack('<Q',len(b)));h.update(b);assert b==(R/name).read_bytes(),name
 assert sha(z.read('fixtures/m0-t13a-acceptance.json'))=='876ea995ea7834bfcea59b2ef8fa56d502aed9006477bcc67bcaa3f5cd944508'
assert h.hexdigest()==info['source_sha256'] and sha((R/'target/release/mycity-headless').read_bytes())==info['binary_sha256']
assert sha((D/'source-support.zip').read_bytes())==info['source_support_sha256']
with zipfile.ZipFile(D/'source-support.zip') as z:
 for name in z.namelist():assert z.read(name)==(R/name).read_bytes()
packets={}
for folder in sorted(D.glob('t13a3d-slots-*')):
 manifest=json.loads((folder/'manifest.json').read_text())
 with zipfile.ZipFile(folder/'artifacts.zip') as z:
  assert set(z.namelist())=={v['path'] for v in manifest}
  for row in manifest:
   b=z.read(row['path']);assert len(b)==row['bytes'] and sha(b)==row['sha256']
  observer=json.loads(z.read('observer-report.json'));report=json.loads(z.read('model/cpu-report.json'))
  assert observer['binary_sha256']==info['binary_sha256'] and report['source_sha256']==info['source_sha256']
 assert not json.loads((folder/'large-files-retained.json').read_text())
 packets[folder.name]={'files':len(manifest),'observer_status':observer['status']}
print(json.dumps({'source_sha256':info['source_sha256'],'binary_sha256':info['binary_sha256'],'source_files_verified':info['files'],'packets':packets,'frozen_acceptance_unchanged':True,'all_payload_in_archive':True,'source_support_verified':True},indent=2))
