#!/usr/bin/env python3
import hashlib,json,struct,zipfile
from pathlib import Path
D=Path(__file__).resolve().parent;R=D.parents[2];load=lambda p:json.loads(p.read_text());sha=lambda b:hashlib.sha256(b).hexdigest()
info=load(D/'source.json');h=hashlib.sha256()
with zipfile.ZipFile(D/'source.zip') as z:
 for n in sorted(z.namelist()):
  b=z.read(n);assert b==(R/n).read_bytes(),n;k=n.encode();h.update(struct.pack('<Q',len(k)));h.update(k);h.update(struct.pack('<Q',len(b)));h.update(b)
 assert len(z.namelist())==info['source_files']==161
assert h.hexdigest()==info['source_sha256']
b=(R/info['binary']).read_bytes();assert sha(b)==info['binary_sha256'] and info['source_sha256'].encode() in b
assert sha((R/'fixtures/m0-t13a-acceptance.json').read_bytes())==info['fixture_sha256']=='876ea995ea7834bfcea59b2ef8fa56d502aed9006477bcc67bcaa3f5cd944508'
for n,h in load(D/'verification-tools.json')['unchanged_copies_sha256'].items():assert sha((D/n).read_bytes())==h
A=D/'t13a70-joint-01';rows=load(A/'manifest.json')
with zipfile.ZipFile(A/'artifacts.zip') as z:
 assert len(z.namelist())==len(rows)
 for row in rows:
  b=z.read(row['path']);assert len(b)==row['bytes'] and sha(b)==row['sha256']
large=load(A/'large-files-retained.json')
for row in large:
 p=R/'work/t13a70-joint-01'/row['path'];assert p.is_file() and p.stat().st_size==row['bytes']
 idx=load(p.parent/'index.json');r=next((r for r in idx['files'] if r['name']==p.name),None)
 if r is None:
  assert p.name=='native-restore-bound.json' and sha(p.read_bytes())==row['sha256']
 else:assert r['bytes']==row['bytes'] and r['sha256']==row['sha256']
checks=load(D/'checks-final.json');assert len(checks)==4 and all(r['exit']==0 for r in checks.values())
print(json.dumps(dict(source_sha256=info['source_sha256'],source_and_binary_match=True,frozen_fixture_and_observer_unchanged=True,archived_files_verified=len(rows),large_files_complete_and_prior_terminal_hash_matches_index=len(large),large_hash_scope='collect.py actually hashed bytes after terminal; here validates preserved files/length and digest against complete snapshot indexes; not a fresh restore',runtime_budget_accepted=False),indent=2))
