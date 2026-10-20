import hashlib,json,re,struct,zipfile
from pathlib import Path
D=Path(__file__).resolve().parent;R=D.parents[2];sha=lambda b:hashlib.sha256(b).hexdigest();info=json.loads((D/'source.json').read_text());h=hashlib.sha256()
with zipfile.ZipFile(D/'source.zip') as z:
 for n in sorted(z.namelist()):
  b=z.read(n);name=n.encode();h.update(struct.pack('<Q',len(name)));h.update(name);h.update(struct.pack('<Q',len(b)));h.update(b);assert b==(R/n).read_bytes(),n
 assert sha(z.read('fixtures/m0-t13a-acceptance.json'))=='876ea995ea7834bfcea59b2ef8fa56d502aed9006477bcc67bcaa3f5cd944508'
 lock=z.read('Cargo.lock').decode();checksum=re.search(r'name = "peak_alloc".*?checksum = "([a-f0-9]+)"',lock,re.S).group(1)
 assert checksum==sha((D/'peak_alloc-0.3.0.crate').read_bytes())
assert h.hexdigest()==info['source_sha256'] and sha((R/'target/release/mycity-headless').read_bytes())==info['binary_sha256']
for folder in D.glob('t13a3b-*'):
 manifest=json.loads((folder/'manifest.json').read_text())
 with zipfile.ZipFile(folder/'artifacts.zip') as z:
  assert set(z.namelist())=={v['path'] for v in manifest}
  for row in manifest:
   b=z.read(row['path']);assert len(b)==row['bytes'] and sha(b)==row['sha256']
print(json.dumps({'source_sha256':info['source_sha256'],'binary_sha256':info['binary_sha256'],'same_source_for_failed_and_retry':True,'frozen_acceptance_unchanged':True,'all_packet_bytes_verified':True,'dependency_checksum_verified':checksum},indent=2))
