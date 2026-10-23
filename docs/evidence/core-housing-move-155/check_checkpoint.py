"""Independent literal checkpoint/cash oracle, not another WAL replay engine."""
from pathlib import Path
import json, hashlib, sys
report_path, world_path = map(Path, sys.argv[1:])
r = json.loads(report_path.read_text())
final = r['stages']['finalcold']
assert r['checkpoint_cold_restore'] == 'passed'
assert r['wages']['day38']['accrued'] == 266000
assert r['wages']['day38']['paid'] == 245000
assert r['wages']['day42']['paid'] == 266000
assert r['wages']['day42_paid_delta'] == 21000
assert r['claims']['old_final']['principal'] == 383226
assert r['claims']['old_final']['paid'] == 341000
assert r['claims']['old_outstanding'] == 42226
assert r['claims']['new_final']['principal'] == 30000
assert r['claims']['new_final']['paid'] == 0
expected_cash = {1:1000000, 2:85000+266000-341000-10000,
                 3:600000-266000+10000, 4:1200000+341000}
assert sum(expected_cash.values()) == 2885000
assert {a['id']:a['cash'] for a in final['accounts']} == expected_cash
assert final['account_cash_total'] == 2885000
assert final['active_city'] == 3
assert len(final['housing_move']['historical_leases']) == 1
assert final['housing_move']['waits'] == []
checkpoints=[]
for path in world_path.glob('checkpoint-*.json'):
    value=json.loads(path.read_bytes())
    if 'engine' in value: checkpoints.append((value['sequence'], value))
seq,cp=max(checkpoints,key=lambda row:row[0])
e=json.loads(bytes(cp['engine'])); w=e['world']
accounts=sorted((s['value'] for s in w['accounts']['slots'] if s['value'] is not None),key=lambda a:a['id'])
assert {a['id']:a['cash'] for a in accounts} == expected_cash
av=[{k:v for k,v in a.items() if k in ['id','role','cash','protected','opening_source_id']} for a in accounts]
sv=[]
for slot in w['stocks']['slots']:
    if slot['value'] is None: continue
    s=slot['value'].copy()
    s['owner']=w['accounts']['slots'][s['owner']['raw']['index']]['value']['id']
    sv.append({k:v for k,v in s.items() if k in ['id','owner','commodity','quantity','basis','lot_id','origin_quantity','origin_basis','offset','opening_source_id','opening_basis']})
v={k:w[k] for k in ['scope','seed','revision','region','economy','boot','exchange','housing']}
v.update(schema='mycity-m0-state-v8', accounts=av, stocks=sorted(sv,key=lambda s:s['id']), opening_sources=w['sources'])
for k in ['construction','town']:
    if k in w: v[k]=w[k]
e['world']=v; e.pop('journal',None)
def digest(value):
    return hashlib.sha256(json.dumps(value,sort_keys=True,separators=(',',':'),ensure_ascii=False).encode()).hexdigest()
expected_hash=digest({'schema':'mycity-m0-store-state-v1','engine':digest(e),'spatial':None})
assert expected_hash == final['state_hash']
assert seq == final['sequence']
print(json.dumps({'passed':True,'scope':'literal cash/claim report and independent canonical checkpoint business digest; no WAL replay',
 'sequence':seq,'store_state_hash':expected_hash,'cash':expected_cash,'cash_total':sum(expected_cash.values()),
 'old_principal':383226,'old_paid':341000,'old_outstanding':42226,'new_unpaid':30000,
 'world':str(world_path),'report':str(report_path)},indent=2))
