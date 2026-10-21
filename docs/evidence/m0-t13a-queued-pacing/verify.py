#!/usr/bin/env python3
"""Verify complete real session trajectory, immutable v1 checkpoint and original receipts."""
import collections,hashlib,json,sys
from pathlib import Path
D=Path(__file__).resolve().parent;R=Path(sys.argv[1]);M=R/'model'
load=lambda p:json.loads(p.read_text());sha=lambda b:hashlib.sha256(b).hexdigest()
info=load(D/'source.json');freeze=load(D/'freeze.json');report=load(M/'report.json');parent=load(M/'supervisor-report.json');o=load(R/'observer-report.json');manifest=load(M/'manifest.json')
assert o['status']=='passed_measurement_wrapper' and o['model_exit_code']==0 and o['binary_unchanged'] and o['observer_source_unchanged']
assert o['binary_sha256']==info['binary_sha256'] and o['observer_source_sha256']==info['observer_sha256']
assert report['source_sha256']==parent['source_sha256']==manifest['source_sha256']==info['source_sha256']
rate=report['rate'];assert rate in ['step','60'] and parent['rate']==manifest['rate']==rate and parent['experiment_completed']
targets=freeze['targets'];assert targets==sorted(set(range(3600,2592001,3600))|{1800,34200}) and len(targets)==722 and manifest['targets']==targets
assert manifest['opening_sha256']==freeze['opening_sha256']
refs=[json.loads(x) for x in (M/'reference-targets.jsonl').read_text().splitlines()];assert [x['target'] for x in refs]==targets
assert refs[0]['raw_in_transit'] and next(x for x in refs if x['target']==34200)['sales_in_transit']
assert next(x for x in refs if x['target']==34200)['consumed_f']==0
pretax=next(x for x in refs if x['target']==2592000-3600);assert pretax['closed_days']==29 and not pretax['tax_claim_exists'];assert refs[-1]['closed_days']==30 and refs[-1]['tax_claim_exists']
rows=[json.loads(x) for x in (M/'transcript.jsonl').read_text().splitlines()]
assert len(rows)==723+(rate=='60') and len((M/'transcript.jsonl').read_bytes())<=8*1024**2
last_time=0
for i,x in enumerate(rows,1):
 assert x['request']['id']==x['response']['id']==i and 'error' not in x['response']
 assert last_time<=x['submitted_seconds']<=x['completed_seconds'];last_time=x['completed_seconds']
commands=[x['request']['request']['command'] for x in rows];assert commands==(['resume'] if rate=='60' else [])+[('run' if rate=='60' else 'step')]*722+['checkpoint']
steps=rows[(rate=='60'):-1];prev=0;original={}
genesis=load(M/'world/genesis.json');assert sha(bytes(genesis['opening']))==freeze['opening_sha256']
for i,(x,t,e) in enumerate(zip(steps,targets,refs),1):
 expected={'world':1,'branch':1,'operation_id':i,'expected_revision':i-1,'actions':[{'kind':'economy','command':{'kind':'advance','seconds':t-prev}}]}
 env=x['request']['request']['envelope'];assert env==expected
 original[i]=hashlib.sha256(json.dumps(expected,separators=(',',':')).encode()).digest()
 r=x['response']['result'];assert r['status']['provenance']['source_sha256']==info['source_sha256'];assert r['outcome']['has_persistent_receipt'] and 'Ok' in r['outcome']['model'] and not r['already_recorded']
 assert r['status']['committed_game_seconds']==t and r['status']['engine_state_sha256']==e['state_sha256'] and r['status']['revision']==r['status']['sequence']==i
 joint={'schema':'mycity-m0-store-state-v1','engine':e['state_sha256'],'spatial':genesis['spatial']}
 assert sha(json.dumps(joint,separators=(',',':'),sort_keys=True).encode())==r['status']['state_sha256']
 prev=t
# Bind all persistent facts to actual CURRENT -> root -> immutable checkpoint chain.
world=M/'world';current=load(world/'CURRENT.json');raw=(world/f"root-{current['slot']}.json").read_bytes();assert sha(raw)==current['root_sha256'];root=json.loads(raw);assert root['sequence']==722
ref=root['checkpoint'];seen={};image=None
while ref:
 raw=(world/f"checkpoint-index-{ref['generation']}.json").read_bytes();assert sha(raw)==ref['sha256'];index=json.loads(raw)
 raw=(world/f"checkpoint-{ref['generation']}.json").read_bytes();assert sha(raw)==index['data_sha256'];cp=json.loads(raw)
 assert cp['previous']==index['previous'] and cp['genesis_sha256']==root['genesis_sha256']
 assert index['operations']==[r['operation_id'] for r in cp['receipts']]
 if image is None:image=bytes(cp['engine']);assert cp['sequence']==722
 for r in cp['receipts']:
  op=r['operation_id'];assert op not in seen and op in original and r['actor']==0 and r['expected_revision']==op-1 and bytes(r['payload_sha256'])==original[op] and 'Ok' in r['outcome'];seen[op]=r
 ref=cp['previous']
assert len(seen)==722 and image==(M/'reference-engine.json').read_bytes()
assert sha(image)==report['final_engine_sha256'] and report['complete_engine_including_metadata']=='equal_direct_engine_same_trajectory' and report['cold_reopen_oldest_and_latest_retry']=='passed'
facts=load(M/'facts.json');assert facts==load(M/'reference-facts.json')
assert facts['cash_cents']==[808000,56000,816000,1320000] and facts['closed_days']==30
for k,v in {'raw_used':300000,'produced':300000,'consumed_f':180000,'consumed_g':120000,'tax_accrued':24000,'supplier_remaining':300000,'claim2_outstanding':0}.items():assert facts[k]==v
assert facts['other_stocks_empty'] and facts['in_transit_empty']
# Independently read the actual stored model, not only driver's summary fields.
w=json.loads(image)['world'];accounts={s['value']['id']:s['value'] for s in w['accounts']['slots'] if s['value'] is not None}
assert [accounts[i]['cash'] for i in [1,2,3,4]]==[808000,56000,816000,1320000]
stocks={s['value']['id']:s['value'] for s in w['stocks']['slots'] if s['value'] is not None}
assert stocks[1]['quantity']==300000 and all(s['quantity']==0 for i,s in stocks.items() if i!=1)
e=w['economy'];assert e['closed_days']==30 and e['raw_used']==300000 and e['produced']==300000 and e['consumed_f']==180000 and e['consumed_g']==120000 and e['tax_accrued']==24000
assert e['raw_transit'] is None and all(x is None for x in e['sales_transit'])
assert w['revision']==722 and e['claims'][2] is not None and e['claims'][2]['principal']==e['claims'][2]['paid']==24000
end=report['operating_seconds'];assert rows[-1]['request']['id']==report['final_checkpoint_request_id'] and rows[-1]['completed_seconds']<=end
assert report['completed_game_seconds']==2592000 and report['completed_targets']==report['original_receipts_verified']==722
days=report['days'];assert len(days)==30;previous=0;met=[]
for n,d in enumerate(days,1):
 assert d['day']==n;completed=d['completed_seconds'];row=next(x for x,t in zip(steps,targets) if t==n*86400)
 assert row['completed_seconds']<=completed
 assert abs(d['adjacent_seconds']-(completed-previous))<1e-8
 if rate=='60':assert d['fixed_start_seconds']==(n-1)*60;measure=completed-(n-1)*60
 else:assert d['fixed_start_seconds'] is None;measure=completed-previous
 assert abs(d['budget_seconds']-measure)<1e-8 and d['budget_met']==(measure<=60);met.append(measure<=60);previous=completed
last_start=days[28]['completed_seconds'] if rate=='step' else 1740
assert abs(report['final_day_with_checkpoint_seconds']-(end-last_start))<1e-8
all_days=all(met) and end-last_start<=60;whole=end<=1800
assert report['all_daily_budgets_met']==all_days and report['whole_30day_budget_met']==whole and report['speed_target_met']==(all_days and whole)
heaps=[json.loads(x) for x in (M/'heaps.jsonl').read_text().splitlines()]
assert len(heaps)==3 and collections.Counter(x['role'] for x in heaps)=={'pe0-speed-probe':1,'pe0-speed-driver':1,'session':1}
assert all(x['success'] and x['source_sha256']==info['source_sha256'] for x in heaps)
assert len({(x['pid'],x['instance_unix_ns']) for x in heaps})==3
for x in heaps:
 if x['role']=='pe0-speed-probe':assert x['heap']['lifetime_peak_requested_bytes']+x['tail_reserved_bytes']<=256*1024
print(json.dumps({'source_sha256':info['source_sha256'],'rate':rate,'full_30day_actual_session':True,'operating_seconds':end,'max_day_budget_seconds':max(d['budget_seconds'] for d in days),'final_day_with_checkpoint_seconds':end-last_start,'daily_gate_met':all_days,'whole_gate_met':whole,'speed_target_met':all_days and whole,'all722_targets_and_original_receipts':True,'exact_same_trajectory_Engine_including_metadata':True,'original_pe0_endpoints':True,'old_half_hour_sequence_recovery':'not_retested','L1':'absent','common_8MiB':'not_accepted','normal_64MiB':'not_accepted','full_L2_four_rates':'not_run','eight_hour':'not_run','real_16GB':'not_run','full_M0':'not_accepted'},indent=2))
