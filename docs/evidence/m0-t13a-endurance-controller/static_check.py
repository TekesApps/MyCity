#!/usr/bin/env python3
import hashlib,json,struct,zipfile
from pathlib import Path
D=Path(__file__).resolve().parent;R=D.parents[2];sha=lambda b:hashlib.sha256(b).hexdigest();known={};capsules=[]
for folder,current in [(D/'attempt1-source',False),(D,True)]:
 info=json.loads((folder/'source.json').read_text());h=hashlib.sha256()
 with zipfile.ZipFile(folder/'source.zip') as z:
  assert len(z.namelist())==info['source_files']
  for n in sorted(z.namelist()):
   p=n.encode();b=z.read(n)
   if current:assert b==(R/n).read_bytes(),n
   h.update(struct.pack('<Q',len(p)));h.update(p);h.update(struct.pack('<Q',len(b)));h.update(b)
  assert sha(z.read('fixtures/m0-t13a-acceptance.json'))==info['frozen_acceptance_sha256']=='876ea995ea7834bfcea59b2ef8fa56d502aed9006477bcc67bcaa3f5cd944508'
 assert h.hexdigest()==info['source_sha256']
 assert sha((folder/'source-support.zip').read_bytes())==info['source_support_sha256']
 for name,key in [('runner.py','runner_sha256'),('observe.py','observer_sha256')]:assert sha((folder/name).read_bytes())==info[key]
 if current:
  binary=(R/'target/release/mycity-headless').read_bytes();assert sha(binary)==info['binary_sha256'] and info['source_sha256'].encode() in binary
 known[info['source_sha256']]=info;capsules.append({'folder':folder.name,'source_sha256':info['source_sha256'],'files':info['source_files'],'matches_current':current})
archives=[]
for p in D.glob('t13a49-*/manifest.json'):
 rows=json.loads(p.read_text())
 with zipfile.ZipFile(p.parent/'artifacts.zip') as z:
  assert set(z.namelist())=={r['path'] for r in rows}
  for r in rows:
   b=z.read(r['path']);assert len(b)==r['bytes'] and sha(b)==r['sha256']
  m=json.loads(z.read('model/manifest.json'));info=known[m['source_sha256']];assert m['rust_binary_sha256']==info['binary_sha256']
 archives.append({'run':p.parent.name,'source_sha256':m['source_sha256'],'files':len(rows)})
p=D/'attempt1-source/regression-failure-manifest.json';rows=json.loads(p.read_text())
with zipfile.ZipFile(p.parent/'regression-failure-artifacts.zip') as z:
 assert set(z.namelist())=={r['path'] for r in rows}
 for r in rows:
  b=z.read(r['path']);assert len(b)==r['bytes'] and sha(b)==r['sha256']
print(json.dumps({'capsules':capsules,'runtime_archives':archives,'failed_regression_files':len(rows),'frozen_load_unchanged':True,'scope':'static provenance only; runtime verdicts remain separate','eight_hour':'not_run','real_16GB':'not_run','full_M0':'not_accepted'},indent=2))
