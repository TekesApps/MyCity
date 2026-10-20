#!/usr/bin/env python3
"""Independent archived v1 chain/state/receipt and disk-growth lower-bound audit."""
import hashlib,json,struct,zipfile
from pathlib import Path
D=Path(__file__).parent
load=lambda n:json.loads((D/n).read_text())
sha=lambda b:hashlib.sha256(b).hexdigest()
h=hashlib.sha256()
with zipfile.ZipFile(D/'source.zip') as z:
 for name in sorted(z.namelist()):
  data=z.read(name);n=name.encode();h.update(struct.pack('<Q',len(n)));h.update(n);h.update(struct.pack('<Q',len(data)));h.update(data)
assert h.hexdigest()==load('source.json')['source_sha256']
b=load('baseline.json');m=load('world-manifest.json');o=load('observer.json')
assert b['source_sha256']==load('source.json')['source_sha256']==load('refactor-reference.json')['source_sha256']
assert b['completed_saves']==19 and b['requested_saves']==96 and b['rejected_save']['requested_save']==20
assert b['final_state_equal'] and not b['business_time_advanced'] and b['ninety_six_save_gate']=='failed_lower_bound'
assert o['model_exit_code']!=0 and o['status']=='model_failed' and o['binary_unchanged']
with zipfile.ZipFile(D/'world.zip') as z:
 assert set(z.namelist())=={f['path'] for f in m['files']}
 for f in m['files']:
  assert z.getinfo(f['path']).file_size==f['bytes'];h=hashlib.sha256()
  with z.open(f['path']) as src:
   while data:=src.read(65536):h.update(data)
  assert h.hexdigest()==f['sha256']
 current=json.loads(z.read('CURRENT.json'));raw=z.read(f'root-{current["slot"]}.json');assert sha(raw)==current['root_sha256'];root=json.loads(raw)
 assert sha(z.read('genesis.json'))==root['genesis_sha256']
 reference=root['checkpoint'];seen=[];receipts=[];sizes=[]
 while reference:
  generation=reference['generation'];seen.append(generation);raw=z.read(f'checkpoint-index-{generation}.json');assert sha(raw)==reference['sha256'];index=json.loads(raw)
  raw=z.read(f'checkpoint-{generation}.json');assert sha(raw)==index['data_sha256'];sizes.append(len(raw));cp=json.loads(raw)
  assert cp['sequence']==root['sequence']==b['committed_startup_commands']==88
  assert cp['genesis_sha256']==root['genesis_sha256'] and cp['chain']==root['wal_chain']
  engine=bytes(cp['engine']);assert sha(engine)==b['original_engine_checkpoint_sha256'] and len(engine)==b['engine_checkpoint_bytes']
  e=json.loads(engine);assert e['world']['accounts']['live']==64 and e['world']['stocks']['live']==128
  assert len(e['timeline']['claims'])==20000 and len(e['timeline']['payroll'])==64
  assert index['operations']==[r['operation_id'] for r in cp['receipts']]
  receipts+=cp['receipts'];assert index['previous']==cp['previous'];reference=index['previous']
 assert len(seen)==19 and sorted(seen)==list(range(89,108))
 assert sorted(r['operation_id'] for r in receipts)==list(range(1,89)) and all('Ok' in r['outcome'] for r in receipts)
 assert max(sizes[:-1])-min(sizes[:-1])<=1  # metadata generation crosses 99; engine bytes are exactly equal
projected=sum(sizes)+(96-19)*min(sizes)
limit=125829120;failed=b['rejected_save']['error'];assert 'Capacity' in failed
assert b['final_status']['archive_bytes']<limit and b['final_status']['archive_bytes']+6332185>limit
assert projected>limit
# Full 30-day direct CPU reference still produces the previous independent business digest.
assert load('refactor-reference.json')['final_checkpoint_sha256']==json.loads((D.parent/'m0-t13a-assembly/reference-result.json').read_text())['final_checkpoint_sha256']
assert load('refactor-reference.json')['business_sha256']=='17b4cbd4271a18e8bd3ec0ddfe1b1bfe2b33081e3a3f87db79ccd7f87464f432'
result={'actual_successful_saves':19,'rejected_request':20,'normal_limit_bytes':limit,'unchanged_engine_bytes':b['engine_checkpoint_bytes'],'repeated_world_checkpoint_bytes':min(sizes),'world_only_96_save_lower_bound_bytes':projected,'all_88_original_success_receipts_preserved':True,'all_19_world_images_exactly_equal':True,'post_refactor_30_day_business_digest_equal':True,'storage_96_gate':'failed_lower_bound','eight_hour':'not_run','real_16gb':'not_run','full_M0':'not_accepted'}
(D/'checks.json').write_text(json.dumps(result,indent=2)+'\n');print(json.dumps(result,indent=2))
