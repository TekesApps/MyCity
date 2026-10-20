#!/usr/bin/env python3
"""Independent raw duration, attempt binding, full-world and frozen-source audit."""
import hashlib,json,struct,zipfile,collections,math
from pathlib import Path
D=Path(__file__).resolve().parent;load=lambda p:json.loads(p.read_bytes());sha=lambda b:hashlib.sha256(b).hexdigest()
classes=['L0_pe0_model_transaction','L0_pe1_model_transaction','L2_claims_batch','L2_payroll_batch','L2_region_exchange_boot_batch','L2_background_capture_admit_cpu','air_source_page','air_cell_page','L1_native_record_index_page','L1_stub_record_index_page']
summary={};h=hashlib.sha256()
with zipfile.ZipFile(D/'source.zip') as z:
 for name in sorted(z.namelist()):
  p=name.encode();b=z.read(name);h.update(struct.pack('<Q',len(p)));h.update(p);h.update(struct.pack('<Q',len(b)));h.update(b)
 assert h.hexdigest()==load(D/'source.json')['source_sha256'];source=h.hexdigest()
 assert sha(z.read('fixtures/m0-t13a-acceptance.json'))=='876ea995ea7834bfcea59b2ef8fa56d502aed9006477bcc67bcaa3f5cd944508'
for folder in sorted(D.glob('t13a2e-*')):
 if not (folder/'artifacts.zip').exists():continue
 totals=collections.defaultdict(list);producers=collections.Counter();attempts=0;files=load(folder/'manifest.json')
 with zipfile.ZipFile(folder/'artifacts.zip') as z:
  assert set(z.namelist())=={x['path'] for x in files}
  for row in files:
   b=z.read(row['path']);assert len(b)==row['bytes'] and sha(b)==row['sha256']
  observer=json.loads(z.read('observer-report.json'))
  if observer['status']!='passed_measurement_wrapper':
   assert folder.name=='t13a2e-full-01' and 'observer hot-data bound' in observer['reason']
   summary[folder.name]={'status':'failed_retained','reason':observer['reason'],'files':len(files),'actual_wall_seconds':observer['actual_wall_seconds'],'no_full_30day_claim':True}
   continue
  reports=[n for n in z.namelist() if '/diagnostics/' in n and n.endswith('/report.json')]
  for n in reports:
   r=json.loads(z.read(n));assert r['source_sha256']==source
   producer=r['producer'];producers[producer]+=1;prefix=n.removesuffix('report.json')
   rows=[json.loads(b) for b in z.read(prefix+'transactions.jsonl').splitlines()]
   begins=[v for v in rows if v.get('phase')=='begin'];ends=[v for v in rows if 'summary' in v or v.get('phase')=='worker_computation_complete']
   assert len(begins)==len(ends);attempts+=len(begins)
   for begin,end in zip(begins,ends):
    if producer=='worker':assert begin['identity']['ticket']==end['identity']['ticket'] and end['sim_admission']=='not_implied'
    else:assert begin['identity'].get('operation',end['operation'])==end['operation']
   for role_index,role in enumerate(r['roles']):
    b=z.read(prefix+role+'/compute-durations.bin');assert len(b)%9==0
    assert len(b)==r['role_compute'][role_index]['raw_bytes']
    rows_by_class=collections.defaultdict(list)
    for off in range(0,len(b),9):
     c=b[off];assert c<len(classes);ns=struct.unpack_from('<Q',b,off+1)[0];rows_by_class[classes[c]].append(ns)
    for c,declared in r['role_compute'][role_index]['classes'].items():
     samples=sorted(rows_by_class[c]);assert declared['samples']==len(samples)
     assert declared['maximum_ns']==max(samples,default=0)
     assert declared['overflow_40ms_samples']==sum(v>40000000 for v in samples)
     for q in [95,99]:
      if samples:
       actual=samples[math.ceil(len(samples)*q/100)-1];bound=declared['p'+str(q)]
       assert actual<=bound['upper_inclusive_ns']
       assert actual>bound['lower_exclusive_ns'] or actual==0
      else:assert declared['p'+str(q)] is None
     stratum=prefix.split('/diagnostics/')[0].split('/')[-1] if producer=='economy' else producer
     totals[(stratum,producer,role,c)].extend(samples)
    previous_end=0
    for begin,end in zip(begins,ends):
     lo=begin['raw_offsets'][role_index];hi=end.get('raw_offsets_end',end.get('raw_offsets'))[role_index]
     assert 0<=lo<=hi<=len(b) and lo%9==hi%9==0
     assert lo==previous_end;previous_end=hi
    assert previous_end==len(b)
   assert r['owned_diagnostics_bytes'] <= (1024**2 if producer=='worker' else 5*1024**2)
  observer=json.loads(z.read('observer-report.json'));assert observer['status']=='passed_measurement_wrapper' and observer['binary_unchanged']
  if 'full' in folder.name:
   r=json.loads(z.read('model/report.json'));assert r['source_sha256']==source and r['compute_diagnostics_enabled']
   assert r['completed_hour_targets']==720 and r['verified_drive_targets']==723 and r['actual_worker_windows']==724 and r['attempt_failures']==0
   assert producers['worker']==724 and producers['sim']==2
   jobs=[]
   for n in reports:
    report=json.loads(z.read(n))
    if report['producer']=='worker':
     rows=[json.loads(v) for v in z.read(n.removesuffix('report.json')+'transactions.jsonl').splitlines()]
     ends=[v for v in rows if v.get('phase')=='worker_computation_complete']
     assert len(ends)==1;jobs.append(ends[0]['identity']['ticket']['job'])
   assert sorted(jobs)==list(range(1,725))
   targets=[json.loads(v) for v in z.read('model/cpu-targets.jsonl').splitlines()]
   trace=[json.loads(v)['data'] for v in z.read('model/transcript.jsonl').splitlines()]
   verified=[v for v in trace if 'verified_target' in v];assert len(verified)==723
   for v,t in zip(verified,targets):
    assert v['expected']==t and v['verified_target']==t['target']
    assert v['actual_business_sha256']==t['business_sha256'] and v['actual_air_sha256']==t['air_sha256']
   def image(read):
    cur=json.loads(read('CURRENT.json'));b=read('root-'+str(cur['slot'])+'.json');assert sha(b)==cur['root_sha256'];root=json.loads(b)
    ref=root['checkpoint_v2'];b=read('world-index-'+str(ref['generation'])+'.json');assert sha(b)==ref['sha256'];m=json.loads(b)
    point=m['current'];b=read('world-'+str(point['generation'])+'.json');assert len(b)==point['bytes'] and sha(b)==point['sha256']
    prev=m['previous'];prior=read('world-'+str(prev['generation'])+'.json');assert len(prior)==prev['bytes'] and sha(prior)==prev['sha256'];assert b==prior
    ref=m['receipts'];last=root['sequence'];ids=set();total=0
    while ref is not None:
     idx_bytes=read('receipt-index-'+str(ref['generation'])+'.json');assert sha(idx_bytes)==ref['sha256'];idx=json.loads(idx_bytes)
     body_bytes=read('receipts-'+str(ref['generation'])+'.json');assert len(body_bytes)==idx['body_bytes'] and sha(body_bytes)==idx['body_sha256'];body=json.loads(body_bytes)
     assert idx['genesis_sha256']==body['genesis_sha256']==root['genesis_sha256']
     assert body['last']==idx['last']==last and body['first']==idx['first'] and body['previous']==idx['previous']
     assert idx['last']-idx['first']+1==len(body['receipts'])==len(idx['operations'])
     assert [v['operation_id'] for v in body['receipts']]==idx['operations']
     for rec in body['receipts']:
      assert rec['operation_id'] not in ids and 'Ok' in rec['outcome'];ids.add(rec['operation_id'])
     last=idx['first']-1;total+=len(body['receipts']);ref=idx['previous']
    assert last==0 and total==root['sequence']==3011 and 1 in ids
    return json.loads(b)
   actual=image(lambda n:z.read('model/world/'+n));cpu=json.loads(z.read('model/cpu-final-engine.json'))
   with zipfile.ZipFile(D.parent/'m0-t13a-tiered/full/world.zip') as old:assert actual==image(old.read)
   with zipfile.ZipFile(D.parent/'m0-t13a-tiered/full/measurements.zip') as old:assert cpu==json.loads(old.read('model/cpu-final-engine.json'))
   assert observer['other_diagnostic_owned_bytes']==6*1024**2
   quota=z.read('model/diagnostics/run-budget.bin');raw_quota,log_quota=struct.unpack('<QQ',quota)
   assert raw_quota<=128*1024**2 and log_quota<=256*1024**2
  else:
   r={};cli=json.loads(z.read('model.stdout'))
   assert cli['source_sha256']==source and cli['economy_probe']['result']=='P-E0_P-E1_executed_and_verified'
 rows=[]
 for (stratum,p,role,c),values in sorted(totals.items()):
  if not values:continue
  values.sort();p95=values[math.ceil(len(values)*.95)-1];p99=values[math.ceil(len(values)*.99)-1]
  rows.append({'stratum':stratum,'producer':p,'role':role,'class':c,'samples':len(values),'p95_ns':p95,'p99_ns':p99,'max_ns':values[-1],
      'diagnostic_threshold':('insufficient_samples' if len(values)<200 else 'within_20_40ms' if p95<=20000000 and p99<=40000000 else 'outside_20_40ms'),
      'formal_gate':'not_accepted; air attempts require outcome selection; repetitions and joint gates outstanding'})
 summary[folder.name]={'files':len(files),'producers':dict(producers),'bound_attempts':attempts,'raw_compute':rows,'sim_work_seconds':r.get('sim_work_wall_seconds_including_ipc_and_persistence'),'full_state_exact_v036':bool(r),'observer_status':observer['status']}
result={'source_sha256':source,'runs':summary,'formal_performance_acceptance':'not_run','eight_hour_gate':'not_run','memory_16gb_gate':'not_run','full_M0':'not_accepted'}
(D/'checks.json').write_text(json.dumps(result,indent=2)+'\n');print(json.dumps(result,indent=2))
