#!/usr/bin/env python3
import hashlib,json,struct,zipfile
from pathlib import Path
D=Path(__file__).resolve().parent;R=D.parents[2];load=lambda p:json.loads(p.read_text());sha=lambda b:hashlib.sha256(b).hexdigest()
info=load(D/'source.json');h=hashlib.sha256()
with zipfile.ZipFile(D/info['source_archive']) as z:
 names=['.cargo/config.toml','Cargo.toml','Cargo.lock','rust-toolchain.toml']
 for folder in ('crates','fixtures'):names.extend(p.relative_to(R).as_posix() for p in (R/folder).rglob('*') if p.is_file())
 assert sorted(z.namelist())==sorted(names)
 for n in sorted(z.namelist()):
  b=z.read(n);assert b==(R/n).read_bytes(),n;p=n.encode();h.update(struct.pack('<Q',len(p)));h.update(p);h.update(struct.pack('<Q',len(b)));h.update(b)
 assert h.hexdigest()==info['source_sha256'] and len(z.namelist())==info['source_files']
binary=(R/'target/release/mycity-headless').read_bytes();assert sha(binary)==info['binary_sha256'] and info['source_sha256'].encode() in binary
assert sha((R/'fixtures/m0-t13a-acceptance.json').read_bytes())==info['frozen_acceptance_sha256']=='876ea995ea7834bfcea59b2ef8fa56d502aed9006477bcc67bcaa3f5cd944508'
assert sha((D/'observe.py').read_bytes())==info['observer_sha256']=='9fcb2290b7837fff0e73eefae0e761237fa90e9c03f8d53d96b8388647bc5ac1'
assert sha((D/'freeze.json').read_bytes())==info['profile_freeze_sha256']
archives=[]
for p in D.glob('t13a56-*/manifest.json'):
 rows=load(p)
 with zipfile.ZipFile(p.parent/'artifacts.zip') as z:
  assert len(rows)==len(z.namelist())
  for x in rows:
   b=z.read(x['path']);assert len(b)==x['bytes'] and sha(b)==x['sha256']
 archives.append({'name':p.parent.name,'files':len(rows)})
print(json.dumps({'source_sha256':info['source_sha256'],'current_source_binary_and_original_acceptance_match':True,'archives':archives,'scope':'static only; not runtime/performance or full joint acceptance'},indent=2))
