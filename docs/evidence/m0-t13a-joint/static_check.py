import hashlib,json,struct,zipfile
from pathlib import Path
D=Path(__file__).resolve().parent;R=D.parents[2];sha=lambda b:hashlib.sha256(b).hexdigest()
result={}
for prefix in ['source','source-reader-b','source-final']:
 info=json.loads((D/(prefix+'.json')).read_text());h=hashlib.sha256()
 with zipfile.ZipFile(D/(prefix+'.zip')) as z:
  for n in sorted(z.namelist()):
   b=z.read(n);name=n.encode();h.update(struct.pack('<Q',len(name)));h.update(name);h.update(struct.pack('<Q',len(b)));h.update(b)
   if prefix=='source-final':assert b==(R/n).read_bytes(),n
  assert sha(z.read('fixtures/m0-t13a-acceptance.json'))=='876ea995ea7834bfcea59b2ef8fa56d502aed9006477bcc67bcaa3f5cd944508'
 assert h.hexdigest()==info['source_sha256'];result[prefix]=h.hexdigest()
 if prefix=='source-final':assert info['binary_sha256']==sha((R/'target/release/mycity-headless').read_bytes())
with zipfile.ZipFile(D/'source-support.zip') as z:
 for n in z.namelist():assert z.read(n)==(R/n).read_bytes()
for folder in D.glob('t13a3a-*'):
 manifest=json.loads((folder/'manifest.json').read_text())
 with zipfile.ZipFile(folder/'artifacts.zip') as z:
  assert set(z.namelist())=={v['path'] for v in manifest}
  for row in manifest:
   b=z.read(row['path']);assert len(b)==row['bytes'] and sha(b)==row['sha256']
print(json.dumps({'source_chain':result,'original_failure_preserved':True,'final_source_matches_worktree':True,'frozen_input_unchanged':True,'outer_archives_verified':True,'source_support_verified':True},indent=2))
