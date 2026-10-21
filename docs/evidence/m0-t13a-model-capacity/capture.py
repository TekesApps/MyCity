#!/usr/bin/env python3
"""Seal this diagnostic's final source/executable and execute its real CLI."""
import hashlib,json,os,struct,subprocess,zipfile
from pathlib import Path
D=Path(__file__).resolve().parent;R=D.parents[2];os.chdir(R)
sha=lambda b:hashlib.sha256(b).hexdigest()
checks=json.loads((D/'checks.json').read_text());assert len(checks)==4 and all(x['exit']==0 for x in checks.values())
names=['.cargo/config.toml','Cargo.toml','Cargo.lock','rust-toolchain.toml']+[str(p) for f in ['crates','fixtures'] for p in Path(f).rglob('*') if p.is_file()]
h=hashlib.sha256()
exists=(D/'source.zip').exists()
with zipfile.ZipFile(D/'source.zip','r' if exists else 'x',zipfile.ZIP_DEFLATED) as z:
 for n in sorted(names):
  b=Path(n).read_bytes();k=n.encode();h.update(struct.pack('<Q',len(k)));h.update(k);h.update(struct.pack('<Q',len(b)));h.update(b)
  if exists:assert z.read(n)==b,n
  else:z.writestr(n,b)
b=Path('target/release/mycity-headless').read_bytes();assert h.hexdigest().encode() in b
exe=R/'work/candidate-binaries/model-capacity/mycity-headless';exe.parent.mkdir(parents=True,exist_ok=True);
if exe.exists():assert sha(exe.read_bytes())==sha(b)
else:os.link('target/release/mycity-headless',exe)
info={'source_sha256':h.hexdigest(),'source_files':len(names),'binary_sha256':sha(b),'binary':str(exe.relative_to(R)),'fixture_sha256':sha(Path('fixtures/m0-t13a-acceptance.json').read_bytes()),'scope':'typed design-capacity inventory CLI only, NOT runtime budget or performance acceptance'}
assert info['fixture_sha256']=='876ea995ea7834bfcea59b2ef8fa56d502aed9006477bcc67bcaa3f5cd944508'
(D/'source.json').write_text(json.dumps(info,indent=2)+'\n')
results={}
for name,args,want in [('plan',['model-capacity-plan'],0),('invalid-args',['model-capacity-plan','unexpected'],2)]:
 p=subprocess.run([str(exe)]+args,stdout=subprocess.PIPE,stderr=subprocess.PIPE);assert p.returncode==want
 (D/(name+'.json' if name=='plan' else name+'.stdout')).write_bytes(p.stdout);(D/(name+'.stderr')).write_bytes(p.stderr)
 results[name]={'args':args,'exit':p.returncode}
(D/'cli-checks.json').write_text(json.dumps(results,indent=2)+'\n')
x=json.loads((D/'plan.json').read_text());assert x['runtime_grant_accepted'] is False and x['core']['runtime_ownership_proven'] is False
for k in ['engine','prepared','extra_air_state']:
 for p in x['core'][k]:assert p['bytes']==p['count']*p['record_bytes']
 assert sum(p['bytes'] for p in x['core'][k])==x['core'][k+'_bytes']
assert sum(p['bytes'] for p in x['metadata'])==x['metadata_bytes']
assert not any(p['name']=='genesis_opening_input' for p in x['metadata'])
assert any(p['name']=='opening_stock_rows' and p['count']==128 for p in x['metadata'])
for s in x['scenarios']:
 assert s['extra_air_slot_bytes']>=x['core']['extra_air_state_bytes']
 assert s['total_bytes']==s['engine_slots']*x['core']['engine_bytes']+s['prepared_slots']*x['core']['prepared_bytes']+s['extra_air_slots']*s['extra_air_slot_bytes']+sum(s[k] for k in ['spatial_resident_bytes','io_codec_bytes','metadata_target_bytes','concurrent_worker_bytes'])
assert max(s['total_bytes'] for s in x['scenarios'])==x['required_bytes']<=x['budget_bytes']==64*1024**2
print(json.dumps({'source':info,'required_bytes':x['required_bytes'],'remaining_bytes':x['budget_bytes']-x['required_bytes'],'runtime_accepted':False,'checks':'real CLI success/error, per-row arithmetic and scenarios recomputed'},indent=2))
