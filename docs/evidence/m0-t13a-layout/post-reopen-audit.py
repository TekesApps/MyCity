#!/usr/bin/env python3
"""Independent same-source continuation audit; streaming, no simulation changes."""
import hashlib,json,struct,sys
from pathlib import Path
r=Path(sys.argv[1]);out=Path(__file__).parent;M=1024**2
assert sorted(p.name for p in r.glob('snapshot-*'))==['snapshot-000003','snapshot-000004']
a=json.loads((r/'snapshot-000003/index.json').read_text());b=json.loads((r/'snapshot-000004/index.json').read_text())
assert a['manifest']['mutation_iteration']==60 and b['manifest']['mutation_iteration']==61
assert a['manifest']['source_sha256']==b['manifest']['source_sha256']
files=[]
for e in b['files']:
 p=r/'snapshot-000004'/e['name'];h=hashlib.sha256()
 with p.open('rb') as f:
  while chunk:=f.read(M):h.update(chunk)
 assert p.stat().st_size==e['bytes'] and h.hexdigest()==e['sha256'];files.append(e)
 if e['name']!='pressure.bin':assert next(x for x in a['files'] if x['name']==e['name'])==e
changed=[];seen=0;state=60+0x4d5943495459;expected=bytearray(M)
for i in range(0,M,8):
 state=(state+0x9e3779b97f4a7c15)&((1<<64)-1);struct.pack_into('<Q',expected,i,(state*0xd1342543de82ef95)&((1<<64)-1))
with (r/'snapshot-000003/pressure.bin').open('rb') as fa,(r/'snapshot-000004/pressure.bin').open('rb') as fb:
 while True:
  x=fa.read(M);y=fb.read(M)
  if not x and not y:break
  assert len(x)==len(y)
  if x!=y:
   changed.append({'offset':seen,'bytes':len(y)});assert seen==60*M and y==expected
  seen+=len(x)
assert changed==[{'offset':60*M,'bytes':M}]
result={'schema':'mycity-independent-layout-continuation-v1','auditor_sha256':hashlib.sha256(Path(__file__).read_bytes()).hexdigest(),'retained_generations':[3,4],'generation_2_retired':True,'all_generation_4_files_independently_hashed':files,'native_stubs_reserve_unchanged':True,'changed_pressure_ranges':changed,'next_dirty_pattern_independently_recomputed':True,'financial_business_proof':False}
(out/'post-reopen-audit.json').write_text(json.dumps(result,indent=2)+'\n');print('PASS: two restore points, exact +1MiB mutation, immutable native/stubs, complete file hashes')
