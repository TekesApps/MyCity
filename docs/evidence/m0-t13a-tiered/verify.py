#!/usr/bin/env python3
"""Independent source/input, original receipts, exact full-state and arithmetic audit."""
import copy,hashlib,json,struct,zipfile
from pathlib import Path
D=Path(__file__).resolve().parent;sha=lambda b:hashlib.sha256(b).hexdigest()
load=lambda p:json.loads(p.read_bytes())
ordered=sorted([60,180,15*86400+120]+[h*3600 for h in range(1,721)])
summary={};h=hashlib.sha256()
with zipfile.ZipFile(D/'source.zip') as source:
 for name in sorted(source.namelist()):
  p=name.encode();b=source.read(name);h.update(struct.pack('<Q',len(p)));h.update(p);h.update(struct.pack('<Q',len(b)));h.update(b)
 source_hash=h.hexdigest();assert source_hash==load(D/'source.json')['source_sha256']
 frozen=json.loads(source.read('fixtures/m0-t13a-tier-manifest.json'))
 assert frozen['acceptance_sha256']==sha(source.read('fixtures/m0-t13a-acceptance.json'))=='876ea995ea7834bfcea59b2ef8fa56d502aed9006477bcc67bcaa3f5cd944508'
 acceptance=json.loads(source.read('fixtures/m0-t13a-acceptance.json'))
 for definition in frozen['tiers']:
  tier=definition['name']
  expected=next(t for t in acceptance['l2_tiers'] if t['name']==('full_current_engine' if tier=='full' else tier))
  assert definition['counts']=={k:expected[k] for k in ['accounts','stocks','payroll','claims']}
  out=D/tier;r=load(out/'model.json');o=load(out/'observer.json');c=load(out/'cpu.json')
  assert r['tier']==c['tier']==tier and r['source_sha256']==c['source_sha256']==source_hash
  for kind,path in definition['files'].items():assert sha(source.read('fixtures/'+path))==definition['sha256'][kind]==r[kind+'_sha256']
  oracle=json.loads(source.read('fixtures/'+definition['files']['oracle']))
  assert r['completed_hour_targets']==720 and r['verified_drive_targets']==723 and r['actual_worker_windows']==724 and r['attempt_failures']==0
  assert r['cold_complete_image_unchanged'] and r['performance_acceptance']==r['steady_three_runs']==r['eight_hour_gate']==r['memory_16gb_gate']=='not_run'
  assert o['binary_unchanged'] and o['observer_source_unchanged'] and o['status']=='passed_measurement_wrapper'
  assert {k:c['counts'][k] for k in definition['counts']}==definition['counts']
  with zipfile.ZipFile(out/'measurements.zip') as z:
   cpu=json.loads(z.read('model/cpu-final-engine.json'))
   targets=[json.loads(b) for b in z.read('model/cpu-targets.jsonl').splitlines()]
   trace=[json.loads(b)['data'] for b in z.read('model/transcript.jsonl').splitlines()]
   assert len(z.read('model/transcript.jsonl'))==r['transcript_bytes']<128*1024**2
  assert [v['target'] for v in targets]==ordered
  verified=[x for x in trace if 'verified_target' in x]
  assert [v['verified_target'] for v in verified]==ordered and [v['expected'] for v in verified]==targets
  commands=[v['command'] for v in trace if 'request' in v]
  assert sum(v['kind']=='status' for v in commands)==726
  assert sum(v['kind']=='run_required' for v in commands)==724
  assert any(v['kind']=='progress' for v in commands)
  with zipfile.ZipFile(out/'world.zip') as z:
   files=load(out/'world-manifest.json');assert set(z.namelist())=={x['path'] for x in files}
   for f in files:
    b=z.read(f['path']);assert len(b)==f['bytes'] and sha(b)==f['sha256']
   cur=json.loads(z.read('CURRENT.json'));b=z.read(f'root-{cur["slot"]}.json');assert sha(b)==cur['root_sha256'];root=json.loads(b)
   assert root==r['final_root'] and root['checkpoint_layout']==2 and root['sequence']==r['saved_sequence']
   assert sha(z.read('genesis.json'))==root['genesis_sha256']
   ref=root['checkpoint_v2'];b=z.read(f'world-index-{ref["generation"]}.json');assert sha(b)==ref['sha256'];manifest=json.loads(b)
   assert manifest['genesis_sha256']==root['genesis_sha256'] and manifest['current']['sequence']==root['sequence']
   images=[]
   for point in [manifest['current'],manifest['previous']]:
    b=z.read(f'world-{point["generation"]}.json');assert len(b)==point['bytes'] and sha(b)==point['sha256'];images.append(b)
   assert images[0]==images[1];actual=json.loads(images[0])
   ids=set();total=0;last=root['sequence'];ref=manifest['receipts'];segments=0
   while ref is not None:
    b=z.read(f'receipt-index-{ref["generation"]}.json');assert sha(b)==ref['sha256'];idx=json.loads(b)
    b=z.read(f'receipts-{ref["generation"]}.json');assert len(b)==idx['body_bytes'] and sha(b)==idx['body_sha256'];body=json.loads(b)
    assert idx['genesis_sha256']==body['genesis_sha256']==root['genesis_sha256']
    assert body['previous']==idx['previous'] and body['first']==idx['first'] and body['last']==idx['last']==last
    assert idx['last']-idx['first']+1==len(body['receipts'])==len(idx['operations'])
    assert [v['operation_id'] for v in body['receipts']]==idx['operations']
    for rec in body['receipts']:
     assert rec['operation_id'] not in ids and 'Ok' in rec['outcome'];ids.add(rec['operation_id'])
    last=idx['first']-1;total+=len(body['receipts']);segments+=1;ref=idx['previous']
   assert last==0 and total==root['sequence'] and 1 in ids
  a=copy.deepcopy(actual);b=copy.deepcopy(cpu);a['world']['revision']=b['world']['revision']=0;a['background']['incarnation']=b['background']['incarnation']=0
  assert a==b
  accounts=[x['value'] for x in actual['world']['accounts']['slots'] if x['value'] is not None]
  assert [{'id':a['id'],'cash':a['cash']} for a in accounts]==oracle['expected_cash_accounts']
  assert sum(a['cash'] for a in accounts)==oracle['expected_cash_total']==3000000
  payroll=actual['timeline']['payroll'];claims=actual['timeline']['claims']
  assert len(payroll)==definition['counts']['payroll'] and len(claims)==definition['counts']['claims']
  assert [p['paid'] for p in payroll]==oracle['expected_payroll_paid']
  assert all(p['work_seconds']==oracle['expected_work_seconds_each'] and p['paid']==p['accrued']==p['billed'] for p in payroll)
  assert all(x['processed'] and x['paid']==x['principal'] for x in claims)
  assert sum(p['paid'] for p in payroll)==definition['wages_paid'] and sum(x['paid'] for x in claims)==definition['claims_paid']
  assert actual['background']['pending'] is None and actual['background']['version']==724 and actual['background']['next_job']==725 and actual['background']['field']['time']==30*86400
  if tier=='full':
   with zipfile.ZipFile(D.parent/'m0-t13a-full-sim/measurements.zip') as old:assert cpu==json.loads(old.read('model/cpu-final-engine.json'))
   with zipfile.ZipFile(D.parent/'m0-t13a-full-sim/world.zip') as old:
    cur=json.loads(old.read('CURRENT.json'));root0=json.loads(old.read(f'root-{cur["slot"]}.json'));ref0=root0['checkpoint_v2'];manifest0=json.loads(old.read(f'world-index-{ref0["generation"]}.json'))
    assert actual==json.loads(old.read(f'world-{manifest0["current"]["generation"]}.json'))
  summary[tier]={'files_checked':len(files),'world_bytes':sum(f['bytes'] for f in files),'complete_state_equal_except_revision_and_incarnation':True,'full_business_oracle':'passed','verified_targets':len(targets),'original_receipts':total,'receipt_segments':segments,'sim_work_seconds':r['sim_work_wall_seconds_including_ipc_and_persistence'],'full_status_calls':sum(v['kind']=='status' for v in commands),'progress_calls':sum(v['kind']=='progress' for v in commands)}
assert all(x['exit_code']==0 for x in load(D/'execution.json'))
summary={'source_sha256':source_hash,'tiers':summary,'performance_acceptance':'not_run','eight_hour_gate':'not_run','memory_16gb_gate':'not_run','full_M0':'not_accepted'}
(D/'checks.json').write_text(json.dumps(summary,indent=2)+'\n');print(json.dumps(summary,indent=2))
