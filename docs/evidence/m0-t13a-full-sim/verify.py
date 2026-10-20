#!/usr/bin/env python3
"""Independent archived full-field, oracle, target and permanent-receipt audit."""
import copy,hashlib,json,struct,zipfile
from pathlib import Path
D=Path(__file__).resolve().parent
load=lambda n:json.loads((D/n).read_bytes())
sha=lambda b:hashlib.sha256(b).hexdigest()
enc=lambda x:json.dumps(x,separators=(',',':'),ensure_ascii=False).encode()
r=load('model.json');o=load('observer.json');c=load('cpu.json')
h=hashlib.sha256()
with zipfile.ZipFile(D/'source.zip') as z:
 for name in sorted(z.namelist()):
  p=name.encode();b=z.read(name);h.update(struct.pack('<Q',len(p)));h.update(p);h.update(struct.pack('<Q',len(b)));h.update(b)
 source=h.hexdigest();assert source==load('source.json')['source_sha256']==r['source_sha256']==c['source_sha256']
 oracle=json.loads(z.read('fixtures/m0-t13a-expected-oracle-candidate.json'))
 assert sha(z.read('fixtures/m0-t13a-acceptance.json'))=='876ea995ea7834bfcea59b2ef8fa56d502aed9006477bcc67bcaa3f5cd944508'
 for n,k in [('full-opening','opening'),('real-policy','policy'),('expected-oracle','oracle')]:
  assert sha(z.read(f'fixtures/m0-t13a-{n}-candidate.json'))==r[k+'_sha256']
assert r['completed_hour_targets']==720 and r['verified_drive_targets']==723 and r['actual_worker_windows']==724
assert r['attempt_failures']==0 and r['cold_complete_image_unchanged']
assert r['performance_acceptance']==r['steady_three_runs']==r['eight_hour_gate']==r['memory_16gb_gate']=='not_run'
assert o['binary_unchanged'] and o['observer_source_unchanged'] and o['status']=='passed_measurement_wrapper'
with zipfile.ZipFile(D/'measurements.zip') as z:
 cpu=json.loads(z.read('model/cpu-final-engine.json'))
 targets=[json.loads(b) for b in z.read('model/cpu-targets.jsonl').splitlines()]
 trace=[json.loads(b)['data'] for b in z.read('model/transcript.jsonl').splitlines()]
 assert len(z.read('model/transcript.jsonl'))==r['transcript_bytes']<128*1024**2
with zipfile.ZipFile(D.parent/'m0-t13a-storage-v2/reference.zip') as old:
 assert cpu==json.loads(old.read('final-engine.json'))
ordered=sorted([60,180,15*86400+120]+[h*3600 for h in range(1,721)])
assert [v['target'] for v in targets]==ordered
verified=[x for x in trace if 'verified_target' in x]
assert [v['verified_target'] for v in verified]==ordered
assert [v['expected'] for v in verified]==targets
with zipfile.ZipFile(D/'world.zip') as z:
 files=load('world-manifest.json');assert set(z.namelist())=={x['path'] for x in files}
 for f in files:
  b=z.read(f['path']);assert len(b)==f['bytes'] and sha(b)==f['sha256']
 cur=json.loads(z.read('CURRENT.json'));b=z.read(f'root-{cur["slot"]}.json');assert sha(b)==cur['root_sha256'];root=json.loads(b)
 assert root==r['final_root'] and root['checkpoint_layout']==2 and root['sequence']==r['saved_sequence']
 assert sha(z.read('genesis.json'))==root['genesis_sha256']
 ref=root['checkpoint_v2'];b=z.read(f'world-index-{ref["generation"]}.json');assert sha(b)==ref['sha256'];manifest=json.loads(b)
 assert manifest['genesis_sha256']==root['genesis_sha256'] and manifest['current']['sequence']==root['sequence']
 for point in [manifest['current'],manifest['previous']]:
  b=z.read(f'world-{point["generation"]}.json');assert len(b)==point['bytes'] and sha(b)==point['sha256']
 actual=json.loads(z.read(f'world-{manifest["current"]["generation"]}.json'))
 assert actual==json.loads(z.read(f'world-{manifest["previous"]["generation"]}.json'))
 ids=set();total=0;last=root['sequence'];ref=manifest['receipts'];segments=0;first_record=None
 while ref is not None:
  b=z.read(f'receipt-index-{ref["generation"]}.json');assert sha(b)==ref['sha256'];idx=json.loads(b)
  b=z.read(f'receipts-{ref["generation"]}.json');assert len(b)==idx['body_bytes'] and sha(b)==idx['body_sha256'];body=json.loads(b)
  assert idx['genesis_sha256']==body['genesis_sha256']==root['genesis_sha256']
  assert body['previous']==idx['previous'] and body['first']==idx['first'] and body['last']==idx['last']==last
  assert idx['last']-idx['first']+1==len(body['receipts'])==len(idx['operations'])
  assert [v['operation_id'] for v in body['receipts']]==idx['operations']
  for rec in body['receipts']:
   assert rec['operation_id'] not in ids;ids.add(rec['operation_id'])
   assert 'Ok' in rec['outcome']
   if rec['operation_id']==1:first_record=rec
  last=idx['first']-1;total+=len(body['receipts']);segments+=1;ref=idx['previous']
 assert last==0 and total==root['sequence'] and first_record is not None
# Every business field is compared; only revision, hot receipts, execution incarnation differ.
a=copy.deepcopy(actual);b=copy.deepcopy(cpu)
a['world']['revision']=b['world']['revision']=0
a['background']['incarnation']=b['background']['incarnation']=0
assert a==b
assert actual['background']['pending'] is None and actual['background']['version']==724 and actual['background']['next_job']==725
assert actual['background']['field']['time']==30*86400
accounts=[x['value'] for x in actual['world']['accounts']['slots'] if x['value'] is not None]
assert [{'id':a['id'],'cash':a['cash']} for a in accounts]==oracle['expected_cash_accounts']
payroll=actual['timeline']['payroll'];claims=actual['timeline']['claims']
assert len(payroll)==64 and len(claims)==20000
assert [p['paid'] for p in payroll]==oracle['expected_payroll_paid']
assert all(p['work_seconds']==oracle['expected_work_seconds_each'] and p['paid']==p['accrued']==p['billed'] for p in payroll)
assert all(x['processed'] and x['paid']==x['principal'] for x in claims)
assert sum(p['paid'] for p in payroll)==118784 and sum(x['paid'] for x in claims)==20000
# Both IPC receipt-proof replies must match the original archived record's exact encoding.
proof={'operation':1,'present':True,'sha256':sha(enc(first_record)),'bytes':len(enc(first_record))}
expected_hash=sha(json.dumps({'Ok':proof},sort_keys=True,separators=(',',':')).encode())
requests=[v['request'] for v in trace if v.get('command',{}).get('kind')=='receipt_proof']
assert len(requests)==2
assert all(next(x for x in trace if x.get('response')==rid and x['kind']=='result')['payload_sha256']==expected_hash for rid in requests)
summary={'source_sha256':source,'files_checked':len(files),'world_bytes':sum(f['bytes'] for f in files),'complete_final_fields_equal_except_revision_and_incarnation':True,'verified_targets':len(targets),'receipt_segments':segments,'permanent_original_receipts':total,'independent_payroll_and_cash_oracle':'passed','performance_acceptance':'not_run','eight_hour_gate':'not_run','memory_16gb_gate':'not_run','full_M0':'not_accepted'}
(D/'checks.json').write_text(json.dumps(summary,indent=2)+'\n');print(json.dumps(summary,indent=2))
