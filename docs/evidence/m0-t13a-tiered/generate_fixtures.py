#!/usr/bin/env python3
"""Freeze legal named tiers and independent arithmetic before any tier performance run.
Default checks byte-for-byte; --write is only for initial fixture creation.
"""
import copy,hashlib,json,sys
from pathlib import Path
R=Path(__file__).resolve().parents[3];F=R/'fixtures'
load=lambda n:json.loads((F/n).read_bytes())
encode=lambda x:(json.dumps(x,ensure_ascii=False,indent=2)+'\n').encode()
full=load('m0-t13a-full-opening-candidate.json');policy=load('m0-t13a-real-policy-candidate.json')
base_oracle=load('m0-t13a-expected-oracle-candidate.json');acceptance=load('m0-t13a-acceptance.json');writes={};tiers=[]
for name,accounts,stocks,payroll,claims in [('small',16,32,8,2000),('medium',32,64,32,10000),('full',64,128,64,20000)]:
 expected=next(t for t in acceptance['l2_tiers'] if t['name']==('full_current_engine' if name=='full' else name))
 assert tuple(expected[k] for k in ['accounts','stocks','payroll','claims'])==(accounts,stocks,payroll,claims)
 o=copy.deepcopy(full);p=copy.deepcopy(policy)
 o['accounts']=o['accounts'][:accounts];o['stocks']=o['stocks'][:stocks]
 used={v['source'] for v in o['accounts']+o['stocks']};o['sources']=[v for v in o['sources'] if v['id'] in used]
 p['initial_account_count']=accounts;p['initial_stock_count']=stocks
 p['payroll']=p['payroll'][:payroll]
 for spec in p['payroll']:spec['payee']=5+(spec['id']-1)%(accounts-4)
 for i,c in enumerate(p['claims']):
  c['count']=claims//20;c['first_id']=i*(claims//20)+1;c['payee']=5+i%(accounts-4)
 cash={v['id']:v['cash'] for v in o['accounts']}
 def pay(a,b,amount):cash[a]-=amount;cash[b]+=amount
 wages=[]
 for spec in p['payroll']:
  wage=29*8*spec['hourly_cents'];wages.append(wage);pay(spec['payer'],spec['payee'],wage)
 for c in p['claims']:pay(c['payer'],c['payee'],c['count']*c['amount'])
 for ob in o['region']['obligations']:pay(ob['payer'],ob['payee'],30*ob['amount'])
 pay(3,4,500)
 for q in o['exchange']['quotes']:
  invoice=q['quantity']*q['unit_price']//1000
  pay(q['buyer'],q['seller'],invoice);pay(q['buyer'],q['carrier'],20);pay(q['buyer'],4,5)
 oracle=copy.deepcopy(base_oracle)
 oracle['expected_payroll_paid']=wages;oracle['expected_wages_paid_total']=sum(wages)
 oracle['expected_claims_paid_total']=claims
 oracle['expected_cash_accounts']=[{'id':a,'cash':v} for a,v in cash.items()]
 oracle['expected_cash_total']=sum(cash.values())
 assert sum(cash.values())==p['initial_cash_cents']==3000000
 assert min(cash.values())>=0
 if name=='full':
  assert o==full and p==policy and oracle==base_oracle
  files={'opening':'m0-t13a-full-opening-candidate.json','policy':'m0-t13a-real-policy-candidate.json','oracle':'m0-t13a-expected-oracle-candidate.json'}
 else:
  files={kind:f'm0-t13a-{name}-{kind}.json' for kind in ['opening','policy','oracle']}
  for kind,value in [('opening',o),('policy',p),('oracle',oracle)]:writes[files[kind]]=encode(value)
 hashes={kind:hashlib.sha256(writes.get(path,(F/path).read_bytes() if (F/path).exists() else b'')).hexdigest() for kind,path in files.items()}
 tiers.append({'name':name,'counts':{'accounts':accounts,'stocks':stocks,'payroll':payroll,'claims':claims},'files':files,'sha256':hashes,'cash_total':sum(cash.values()),'wages_paid':sum(wages),'claims_paid':claims})
manifest={'schema':'mycity-t13a-named-tiers-v1','acceptance_sha256':hashlib.sha256((F/'m0-t13a-acceptance.json').read_bytes()).hexdigest(),'unchanged_common':{'cities':8,'resources':8,'periodic_obligations':16,'freight_quotes':8,'maximum_simultaneous_freight':4,'hours':720,'drive_targets':723,'air_windows':724,'timeline_page_limit':8192,'air_step_seconds':60},'generation_only':'static legal fixtures; runtime never truncates full records; household payees wrap within each named roster','tiers':tiers}
writes['m0-t13a-tier-manifest.json']=encode(manifest)
for name,b in writes.items():
 if '--write' in sys.argv:(F/name).write_bytes(b)
 else:assert (F/name).read_bytes()==b,name
print(json.dumps({'named_tiers':len(tiers),'full_input_and_oracle_unchanged':True,'files_checked':len(writes),'total_cash_each':3000000},indent=2))
