#!/usr/bin/env python3
"""Verify frozen source, observer and archives; no runtime or performance claim."""
import hashlib,json,struct,zipfile
from pathlib import Path
D=Path(__file__).resolve().parent;R=D.parents[2];load=lambda p:json.loads(p.read_text());sha=lambda b:hashlib.sha256(b).hexdigest();info=load(D/'source.json')
with zipfile.ZipFile(D/'source.zip') as z:
 h=hashlib.sha256()
 for n in sorted(z.namelist()):
  b=z.read(n);assert b==(R/n).read_bytes(),n;k=n.encode();h.update(struct.pack('<Q',len(k)));h.update(k);h.update(struct.pack('<Q',len(b)));h.update(b)
 assert h.hexdigest()==info['source_sha256'] and len(z.namelist())==info['source_files']
binary=(R/'target/release/mycity-headless').read_bytes();assert sha(binary)==info['binary_sha256'] and info['source_sha256'].encode() in binary
assert sha((D/'observe.py').read_bytes())==info['observer_sha256']=='9fcb2290b7837fff0e73eefae0e761237fa90e9c03f8d53d96b8388647bc5ac1'
assert sha((R/'fixtures/m0-t13a-acceptance.json').read_bytes())==info['frozen_acceptance_sha256']=='876ea995ea7834bfcea59b2ef8fa56d502aed9006477bcc67bcaa3f5cd944508'
assert sha((D/'source-support.zip').read_bytes())==info['source_support_sha256']
archives=[]
for p in D.glob('t13a51-*/manifest.json'):
 rows=load(p)
 with zipfile.ZipFile(p.parent/'artifacts.zip') as z:
  assert len(rows)==len(z.namelist())
  for x in rows:
   b=z.read(x['path']);assert len(b)==x['bytes'] and sha(b)==x['sha256']
 archives.append({'name':p.parent.name,'files':len(rows)})
print(json.dumps({'source_sha256':info['source_sha256'],'source_and_binary_match':True,'frozen_acceptance_unchanged':True,'observer_unchanged':True,'archives':archives,'scope':'static integrity only; no runtime/performance acceptance'},indent=2))
