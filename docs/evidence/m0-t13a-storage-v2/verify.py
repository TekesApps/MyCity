#!/usr/bin/env python3
"""Independent raw-world, original-receipt, exact-field and save-growth audit."""
import hashlib,json,struct,zipfile
from pathlib import Path
D=Path(__file__).parent;old=D.parent/'m0-t13a-storage'
load=lambda n:json.loads((D/n).read_text());sha=lambda b:hashlib.sha256(b).hexdigest()
h=hashlib.sha256()
with zipfile.ZipFile(D/'source.zip') as z:
 for name in sorted(z.namelist()):
  b=z.read(name);p=name.encode();h.update(struct.pack('<Q',len(p)));h.update(p);h.update(struct.pack('<Q',len(b)));h.update(b)
source=h.hexdigest();assert source==load('source.json')['source_sha256']
reports={}
for prefix in ['borrowed-v1','v2']:
 r=load(prefix+'-model.json');o=load(prefix+'-observer.json');reports[prefix]=r
 assert r['source_sha256']==source and r['requested_saves']==96 and r['final_state_equal'] and not r['business_time_advanced']
 assert r['committed_startup_commands']==88 and r['payroll']==64 and r['claims']==20000
 assert o['binary_unchanged'] and o['observer_source_unchanged'] and o['memory_16gb_gate']=='not_run'
 with zipfile.ZipFile(D/(prefix+'-world.zip')) as z:
  m=load(prefix+'-world-manifest.json');assert set(z.namelist())=={i['path'] for i in m}
  for f in m:
   h=hashlib.sha256()
   with z.open(f['path']) as p:
    while b:=p.read(65536):h.update(b)
   assert z.getinfo(f['path']).file_size==f['bytes'] and h.hexdigest()==f['sha256']
  current=json.loads(z.read('CURRENT.json'));raw=z.read(f'root-{current["slot"]}.json');assert sha(raw)==current['root_sha256'];root=json.loads(raw)
  assert sha(z.read('genesis.json'))==root['genesis_sha256'] and root['sequence']==88
  if prefix=='borrowed-v1':
   assert r['completed_saves']==19 and r['rejected_save']['requested_save']==20 and o['status']=='model_failed'
   assert r['final_status']['archive_bytes']==119735224
   c=json.loads(z.read(f'checkpoint-{root["checkpoint"]["generation"]}.json'));initial=bytes(c['engine'])
  else:
   assert r['checkpoint_layout']==root['checkpoint_layout']==2 and r['completed_saves']==96 and r['rejected_save'] is None and o['status']=='passed_measurement_wrapper'
   assert len(r['saves'])==96
   for n,s in enumerate(r['saves'],1):assert s['save']==n and s['status']['generation']==88+n and s['status']['sequence']==88 and s['status']['hot_receipts']==0
   assert max(s['status']['archive_bytes'] for s in r['saves'])<4*1024**2
   ref=root['checkpoint_v2'];raw=z.read(f'world-index-{ref["generation"]}.json');assert sha(raw)==ref['sha256'];manifest=json.loads(raw)
   assert manifest['genesis_sha256']==root['genesis_sha256'] and manifest['current']['generation']==184 and manifest['previous']['generation']==183
   assert set(n for n in z.namelist() if n.startswith('world-') and not n.startswith('world-index-'))=={'world-183.json','world-184.json'}
   for point in [manifest['current'],manifest['previous']]:
    b=z.read(f'world-{point["generation"]}.json');assert len(b)==point['bytes'] and sha(b)==point['sha256'] and b==initial
    e=json.loads(b);assert e['world']['accounts']['live']==64 and e['world']['stocks']['live']==128 and len(e['timeline']['payroll'])==64 and len(e['timeline']['claims'])==20000
   ref=manifest['receipts'];expected=88;all_records=[]
   while ref:
    raw=z.read(f'receipt-index-{ref["generation"]}.json');assert sha(raw)==ref['sha256'];i=json.loads(raw)
    raw=z.read(f'receipts-{ref["generation"]}.json');assert sha(raw)==i['body_sha256'] and len(raw)==i['body_bytes'];b=json.loads(raw)
    assert b['genesis_sha256']==i['genesis_sha256']==root['genesis_sha256'] and b['first']==i['first'] and b['last']==i['last']==expected
    assert b['last']-b['first']+1==len(b['receipts'])==len(i['operations'])
    assert [x['operation_id'] for x in b['receipts']]==i['operations'];all_records+=b['receipts'];expected=i['first']-1;assert b['previous']==i['previous'];ref=i['previous']
   assert expected==0 and sorted(x['operation_id'] for x in all_records)==list(range(1,89))
   assert len([n for n in z.namelist() if n.startswith('receipts-')])==1 # 95 empty windows made no empty segment
   assert sum(x['bytes'] for x in load('v2-world-manifest.json'))==r['final_status']['archive_bytes']==3872163
# v2 and borrowed-v1 change storage representation, never the actual initial full model.
with zipfile.ZipFile(old/'world.zip') as z:
 c=json.loads(z.read('CURRENT.json'));root=json.loads(z.read(f'root-{c["slot"]}.json'));cp=json.loads(z.read(f'checkpoint-{root["checkpoint"]["generation"]}.json'));assert json.loads(bytes(cp['engine']))==json.loads(initial)
with zipfile.ZipFile(D/'reference.zip') as z:after=json.loads(z.read('final-engine.json'))
with zipfile.ZipFile(D.parent/'m0-t13a-assembly/reference.zip') as z:before=json.loads(z.read('final-engine.json'))
assert after==before # ALL fields, not merely aggregate money or population
assert load('reference.json')['source_sha256']==source and load('reference.json')['business_sha256']=='17b4cbd4271a18e8bd3ec0ddfe1b1bfe2b33081e3a3f87db79ccd7f87464f432'
result={'source_sha256':source,'same_full_initial_fields_and_all_88_original_receipts':True,'full_30_day_checkpoint_fields_equal':True,'raw_encoding_order_changed':True,'borrowed_v1_completed_saves':19,'v2_completed_saves':96,'v2_final_actual_bytes':3872163,'v2_retained_world_generations':[183,184],'v2_permanent_receipt_segments':1,'gate':'paused_storage_lower_bound_passed_only','real_business_96_saves':'not_run','steady':'not_run','eight_hour':'not_run','real_16gb':'not_run','full_M0':'not_accepted'}
(D/'checks.json').write_text(json.dumps(result,indent=2)+'\n');print(json.dumps(result,indent=2))
