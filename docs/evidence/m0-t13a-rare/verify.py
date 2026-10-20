#!/usr/bin/env python3
"""Independent archived facts and raw sample audit; never drives gameplay."""
import collections,hashlib,io,json,math,struct,zipfile
from pathlib import Path
D=Path(__file__).resolve().parent
sha=lambda b:hashlib.sha256(b).hexdigest();load=lambda p:json.loads(p.read_bytes())
classes=['L0_pe0_model_transaction','L0_pe1_model_transaction','L2_claims_batch','L2_payroll_batch','L2_region_exchange_boot_batch','L2_background_capture_admit_cpu','air_source_page','air_cell_page','L1_native_record_index_page','L1_stub_record_index_page']
def image_and_receipts(read):
 cur=json.loads(read('CURRENT.json'));b=read('root-'+str(cur['slot'])+'.json');assert sha(b)==cur['root_sha256'];root=json.loads(b)
 assert root['checkpoint_layout']==2 and root['worker_cache_bytes']==6*1024**2
 ref=root['checkpoint_v2'];b=read('world-index-'+str(ref['generation'])+'.json');assert sha(b)==ref['sha256'];m=json.loads(b)
 images=[]
 for p in [m['current'],m['previous']]:
  if p is None:continue # first v2 snapshot has no previous image; trials still require one below
  b=read('world-'+str(p['generation'])+'.json');assert len(b)==p['bytes'] and sha(b)==p['sha256'];images.append(b)
 ref=m['receipts'];last=root['sequence'];ids=set();total=0
 while ref is not None:
  b=read('receipt-index-'+str(ref['generation'])+'.json');assert sha(b)==ref['sha256'];idx=json.loads(b)
  b=read('receipts-'+str(ref['generation'])+'.json');assert len(b)==idx['body_bytes'] and sha(b)==idx['body_sha256'];body=json.loads(b)
  assert body['genesis_sha256']==idx['genesis_sha256']==root['genesis_sha256']
  assert body['first']==idx['first'] and body['last']==idx['last']==last and body['previous']==idx['previous']
  assert idx['last']-idx['first']+1==len(body['receipts'])==len(idx['operations'])
  assert [r['operation_id'] for r in body['receipts']]==idx['operations']
  for r in body['receipts']:assert r['operation_id'] not in ids and 'Ok' in r['outcome'];ids.add(r['operation_id'])
  last=idx['first']-1;total+=len(body['receipts']);ref=idx['previous']
 assert last==0 and total==root['sequence']
 return images,root,total
source=load(D/'source.json');h=hashlib.sha256()
with zipfile.ZipFile(D/'source.zip') as z:
 for n in sorted(z.namelist()):
  p=n.encode();b=z.read(n);h.update(struct.pack('<Q',len(p)));h.update(p);h.update(struct.pack('<Q',len(b)));h.update(b)
 assert h.hexdigest()==source['source_sha256']
 assert sha(z.read('fixtures/m0-t13a-acceptance.json'))=='876ea995ea7834bfcea59b2ef8fa56d502aed9006477bcc67bcaa3f5cd944508'
results={}
for folder in sorted(D.glob('t13a2f-*')):
 if not (folder/'artifacts.zip').exists():continue
 hist=collections.defaultdict(collections.Counter);manifest=load(folder/'manifest.json')
 with zipfile.ZipFile(folder/'artifacts.zip') as z:
  assert set(z.namelist())=={x['path'] for x in manifest}
  for f in manifest:
   b=z.read(f['path']);assert len(b)==f['bytes'] and sha(b)==f['sha256']
  observer=json.loads(z.read('observer-report.json'))
  if folder.name=='t13a2f-full-01':
   assert observer['status']=='failed' and 'No such file or directory' in observer['reason']
   results[folder.name]={'status':'failed_retained','reason':observer['reason'],'completed':json.loads(z.read('model/report.json'))['completed'],'archives':sum(n.startswith('model/archives/') and n.endswith('.zip') for n in z.namelist()),'directory_durability_fix_not_present':True};continue
  assert observer['status']=='passed_measurement_wrapper' and observer['binary_unchanged'] and observer['observer_source_unchanged']
  if folder.name=='t13a2f-pilot-01':
   results[folder.name]={'status':'functional_pilot_before_directory_durability_fix','completed':2};continue
  summary=json.loads(z.read('model/report.json'));plan=json.loads(z.read('model/run-plan.json'));anchors=json.loads(z.read('model/anchors/report.json'))
  assert summary['source_sha256']==anchors['source_sha256']==source['source_sha256']
  trials=[json.loads(b) for b in z.read('model/trials.jsonl').splitlines()]
  assert len(trials)==summary['completed']==sum(plan['strata'].values())
  assert [v['trial'] for v in trials]==[f'{kind}-{i:03d}' for kind,count in plan['strata'].items() for i in range(count)]
  actual_raw=0;actual_log=0;receipts=0;per_stratum=collections.Counter()
  for trial in trials:
   name='model/archives/'+trial['trial']+'.zip';b=z.read(name);assert len(b)==trial['archive_bytes'] and sha(b)==trial['archive_sha256']
   kind=trial['stratum'];expected=z.read(f'model/anchors/{kind}/expected-engine.json')
   base,_r,_n=image_and_receipts(lambda n:z.read(f'model/anchors/{kind}/world/'+n))
   with zipfile.ZipFile(io.BytesIO(b)) as a:
    m=json.loads(a.read('archive-manifest.json'));assert set(a.namelist())=={x['path'] for x in m}|{'archive-manifest.json'}
    sizes={'raw':0,'log':0,'model':0}
    for f in m:
     p=a.read(f['path']);assert len(p)==f['bytes'] and sha(p)==f['sha256']
     bucket='raw' if f['path'].endswith('/compute-durations.bin') else 'model' if f['path'].startswith('world/') else 'log';sizes[bucket]+=len(p)
    assert sizes==trial['uncompressed_sizes'];actual_raw+=sizes['raw'];actual_log+=sizes['log']
    assert sizes['raw']<=plan['raw_allowance_per_trial'] and sizes['log']<=plan['log_allowance_per_trial']
    images,root,n=image_and_receipts(lambda n:a.read('world/'+n));receipts+=n
    assert images[0]==expected and images[1]==base[0]
    assert sha(expected)==trial['report']['actual_engine_sha256']==trial['report']['expected_engine_sha256']==trial['restore_proof']['actual_engine_sha256']
    assert trial['restore_proof']['cold_archive_restore']=='passed' and trial['restore_proof']['sequence']==root['sequence']
    engine=json.loads(expected);assert len(engine['timeline']['claims'])==20000 and len(engine['timeline']['payroll'])==64
    expected_paid=1000 if kind=='claims' else 0
    assert sum(v['paid'] for v in engine['timeline']['claims'])==expected_paid
    reports=[n for n in a.namelist() if n.startswith('diagnostics/') and n.endswith('/report.json')];assert len(reports)==(3 if kind=='source' else 2) # generator completes at120: source interval has two real worker jobs
    sample_counts={'claims':0,'sim_source':0,'worker_source':0}
    for rp in reports:
     r=json.loads(a.read(rp));assert r['source_sha256']==source['source_sha256'];producer=r['producer'];prefix=rp.removesuffix('report.json')
     rows=[json.loads(v) for v in a.read(prefix+'transactions.jsonl').splitlines()]
     begins=[v for v in rows if v.get('phase')=='begin'];ends=[v for v in rows if 'summary' in v or v.get('phase')=='worker_computation_complete'];assert len(begins)==len(ends)
     for role_index,role in enumerate(r['roles']):
      raw=a.read(prefix+role+'/compute-durations.bin');assert len(raw)%9==0 and len(raw)==r['role_compute'][role_index]['raw_bytes'];counts=collections.Counter()
      for off in range(0,len(raw),9):
       c=raw[off];ns=struct.unpack_from('<Q',raw,off+1)[0];counts[classes[c]]+=1;hist[(kind,producer,role,classes[c])][ns]+=1
      for c,v in r['role_compute'][role_index]['classes'].items():assert v['samples']==counts[c]
      prev=0
      for begin,end in zip(begins,ends):
       lo=begin['raw_offsets'][role_index];hi=end.get('raw_offsets_end',end.get('raw_offsets'))[role_index];assert lo==prev and 0<=lo<=hi<=len(raw) and lo%9==hi%9==0;prev=hi
       if producer=='worker':assert begin['identity']['ticket']==end['identity']['ticket']
      assert prev==len(raw)
      if role=='execution':
       sample_counts['claims']+=counts['L2_claims_batch']
       sample_counts[producer+'_source']+=counts['air_source_page']
     if producer=='sim' and kind=='source':assert r['air_page_limit_bounds_role_source_cell_min_max'][0][0]==[8192,8192]
     if producer=='worker' and kind=='source':assert r['air_page_limit_bounds_role_source_cell_min_max'][0][0]==[256,256]
    assert sample_counts==trial['samples']==({'claims':1,'sim_source':0,'worker_source':0} if kind=='claims' else {'claims':0,'sim_source':2,'worker_source':2})
   per_stratum[kind]+=1
  assert dict(per_stratum)==summary['by_stratum']==plan['strata']
  assert actual_raw==summary['actual_raw_bytes_archived']<=128*1024**2 and actual_log==summary['actual_log_bytes_archived']<=256*1024**2
  rows=[]
  for (kind,producer,role,c),counter in sorted(hist.items()):
   n=sum(counter.values())
   def quantile(q):
    rank=math.ceil(n*q/100);seen=0
    for value,count in sorted(counter.items()):
     seen+=count
     if seen>=rank:return value
   p95=quantile(95);p99=quantile(99)
   rows.append({'stratum':kind,'producer':producer,'role':role,'class':c,'samples':n,'p95_ns':p95,'p99_ns':p99,'max_ns':max(counter),'sample_gate':('insufficient' if n<200 else 'within_20_40ms' if p95<=20000000 and p99<=40000000 else 'outside_20_40ms'),'scope':'rare class repetitions only; no joint steady acceptance'})
  results[folder.name]={'status':'independent_archive_and_raw_audit_passed','completed':len(trials),'by_stratum':dict(per_stratum),'raw_bytes':actual_raw,'log_bytes':actual_log,'original_receipts_checked_across_independent_worlds':receipts,'raw_compute':rows,'whole_wall_seconds':observer['actual_wall_seconds'],'observer_hot_bytes':observer['observer_hot_bound_bytes'],'missing_footprint_samples':observer['missing_footprint_process_samples']}
result={'source_sha256':source['source_sha256'],'runs':results,'formal_10class_gate':'not_accepted_here','three_steady_runs':'not_run','eight_hour_gate':'not_run','memory_16gb_gate':'not_run','full_M0':'not_accepted'}
(D/'checks.json').write_text(json.dumps(result,indent=2)+'\n');print(json.dumps(result,indent=2))
