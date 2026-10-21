#!/usr/bin/env python3
"""Offline raw-range audit. Never infers missing clocks or pools retries/quotes.
Usage: window_audit.py STEADY_RUN_MODEL_DIR
Every byte is accounted, even out-of-window, interrupted or uncertain batches.
Not an RSS/footprint, queue, disk, 3-round or complete performance acceptance.
"""
import collections,json,re,struct,sys
from pathlib import Path
CLASSES=['L0_pe0_model_transaction','L0_pe1_model_transaction','L2_claims_batch','L2_payroll_batch','L2_region_exchange_boot_batch','L2_background_capture_admit_cpu','air_source_page','air_cell_page','L1_native_record_index_page','L1_stub_record_index_page']
ROLES=['execution','quote','replay']
def load(p):return json.loads(p.read_text())
def rows(p):
 with p.open() as f:
  for line in f:
   assert len(line)<=131072,(p,'row bound')
   yield json.loads(line)
def identity(c):return c['pid'],c['instance_unix_ns']
def ticket(t):return json.dumps(t,sort_keys=True,separators=(',',':'))
def intersect(old,new):
 if old is None:return new
 out=max(old[0],new[0]),min(old[1],new[1]);assert out[0]<=out[1],('inconsistent causal clocks',old,new)
 return out
def disposition(bounds,w0,w1):
 if bounds is None:return 'unknown_clock_or_incomplete'
 start_min,start_max,end_min,end_max=bounds
 assert start_min<=start_max and end_min<=end_max and start_min<=end_max
 if start_min>=w0 and end_max<=w1:return 'inside'
 if end_max<=w0:return 'before'
 if start_min>=w1:return 'after'
 return 'cross_boundary_or_uncertain'
def clock_bounds(a,b,origins):
 assert identity(a)==identity(b) and a['elapsed_ns']<=b['elapsed_ns']
 o=origins.get(identity(a))
 return None if o is None else (o[0]+a['elapsed_ns'],o[1]+a['elapsed_ns'],o[0]+b['elapsed_ns'],o[1]+b['elapsed_ns'])
def statistics(h):
 n=sum(h.values());ordered=sorted(h.items())
 def quantile(p):
  rank=(n*p+99)//100;count=0
  for value,k in ordered:
   count+=k
   if count>=rank:return value
  return None
 p95=quantile(95);p99=quantile(99)
 return dict(samples=n,maximum_ns=max(h) if n else None,p95_ns=p95,p99_ns=p99,minimum_samples_met=n>=200,thresholds_met=None if n<200 else p95<=20_000_000 and p99<=40_000_000)
class Audit:
 def __init__(self,root):
  self.root=root;self.b=root/'business';self.report=load(self.b/'report.json');self.source=self.report['source_sha256']
  self.w0=round(self.report['warmup_seconds']*1e9);self.w1=round(self.report['fixed_window_end_seconds']*1e9)
  self.origins={};self.workers={};self.filled=set();self.hist=collections.defaultdict(collections.Counter);self.files={};self.raw_counts=collections.defaultdict(collections.Counter);self.raw_max=collections.defaultdict(lambda:collections.defaultdict(int));self.batches=collections.Counter();self.unmapped=[]
 def clocks(self):
  pool=self.root/'normal-pool/events.jsonl'
  if pool.exists():
   for row in rows(pool):
    if row['kind']=='worker_actual_slots_filled':
     i=row['worker_identity'];self.filled.add((i['process_pid'],i['process_instance_unix_ns'],i['run_epoch'],i['job'],i['generation'],i['attempt']))
  for row in rows(self.b/'window-events.jsonl'):
   if row['kind']=='case_clock_map':
    d=row['data'];k=(d['pid'],d['instance_unix_ns']);o=(d['child_origin_driver_ns_min'],d['child_origin_driver_ns_max']);self.origins[k]=intersect(self.origins.get(k),o)
  for p in self.b.rglob('transcript.jsonl'):
   # Only actual measured city; warmup is separate and carries explicit driver offset.
   offset=None;request_workers=collections.defaultdict(list);request_end={}
   for row in rows(p):
    d=row['data']
    if 'experiment_origin_measurement_ns' in d:offset=self.w0+d['experiment_origin_measurement_ns']
   if offset is None:
    continue
   for row in rows(p):
    d=row['data'];c=d.get('sim_clock')
    if c is None:continue  # No fallback from old Event/PID/finish-time.
    k=identity(c);o=(offset+d['request_sent_ns']-c['elapsed_ns'],offset+d['response_received_ns']-c['elapsed_ns'])
    self.origins[k]=intersect(self.origins.get(k),o)
    wid=d.get('worker_identity')
    if wid:
     request_workers[d['response']].append((wid,offset+d['request_sent_ns']))
    if d['kind']=='result':request_end[d['response']]=(offset+d['response_received_ns'],d.get('result_ok'))
   for req,workers in request_workers.items():
    ending=request_end.get(req)
    for i,(wid,start) in enumerate(workers):
     key=(wid['worker_pid'],ticket(wid['ticket']));assert key not in self.workers
     self.workers[key]=dict(bounds=None if ending is None else (start,start,ending[0],ending[0]),admission='accepted_request_last_attempt' if ending and ending[1] is True and i==len(workers)-1 else 'failed_or_unconfirmed_attempt')
  # Warmup clocks intentionally need not resolve: their raw ranges are still accounted as unknown.
 def segment(self,p,start,end,tag,air_spec=None):
  assert 0<=start<=end<=p.stat().st_size and start%9==end%9==0,(p,start,end)
  old=self.files.get(str(p),0);assert start==old,('gap/duplicate raw interval',p,start,old)
  self.files[str(p)]=end
  if start==end:return
  self.batches[tag]+=1
  with p.open('rb') as f:
   f.seek(start);left=end-start
   while left:
    b=f.read(min(left,9*8192));assert b and len(b)%9==0;left-=len(b)
    for c,ns in struct.iter_unpack('<BQ',b):
     assert c<len(CLASSES)
     self.raw_counts[str(p)][CLASSES[c]]+=1;self.raw_max[str(p)][CLASSES[c]]=max(self.raw_max[str(p)][CLASSES[c]],ns)
     spec='not_air'
     if c in (6,7):
      bounds=None if air_spec is None else air_spec[c-6]
      spec='unproven_air_spec' if not bounds or bounds[0]==0 or bounds[0]!=bounds[1] else 'page_limit_'+str(bounds[0])
     h=self.hist[(*tag,CLASSES[c],spec)];h[ns]+=1
     assert len(h)<=1_000_000,'offline distinct-duration guard'
 def producers(self):
  for p in sorted(self.b.rglob('transactions.jsonl')):
   first=next(rows(p),{});ident=first.get('identity')
   if not ident or first.get('phase')!='producer_identity':continue
   assert ident['source_sha256']==self.source
   k=(ident['process_pid'],ident['process_instance_unix_ns']);kind=ident['producer'];sealed=(p.parent/'report.json').exists()
   if sealed:
    sealed_report=load(p.parent/'report.json');assert sealed_report['source_sha256']==self.source
   rel=p.relative_to(self.b).parts
   if re.fullmatch(r'(?:warmup|measurement)-[0-9]{5}-(?:economy|boot|exchange)',rel[0]):
    case=rel[0].rsplit('-',1)[1]+'/'+('/'.join(rel[1:rel.index('diagnostics')]))
   elif rel[0]=='measurement':case='L2/full_original'
   elif re.fullmatch(r'warmup-[0-9]{4}',rel[0]):case='L2/full_warmup'
   else:case='unproven_case/'+str(p.parent.relative_to(self.b))
   consumed=[0,0,0];begin=None
   for row in rows(p):
    c=row.get('process_clock')
    if c is not None:assert identity(c)==k
    if row.get('phase')=='begin':
     assert row['raw_offsets']==consumed,('unclosed predecessor',p)
     begin=row
    finish=row.get('raw_offsets_end')
    worker_complete=row.get('phase')=='worker_computation_complete'
    if worker_complete:finish=row['raw_offsets']
    if finish is None:continue
    assert begin is not None,(p,'end without begin')
    bounds=None;admission='unconfirmed'
    if worker_complete:
     key=(k[0],ticket(begin['identity']['ticket']));wr=self.workers.get(key)
     t=begin['identity']['ticket']
     exact=(k[0],k[1],t['incarnation'],t['job'],t['generation'],t['attempt'])
     if wr and exact in self.filled:bounds=wr['bounds'];admission=wr['admission']
    else:
     if begin.get('process_clock') is not None and c is not None:
      bounds=clock_bounds(begin['process_clock'],c,self.origins)
     sm=row['summary'];admission='confirmed_applied' if row['durable_receipt'] and sm['applied'] and sm['hook_succeeded'] and not sm['replayed'] else 'failed_replayed_or_quote'
    window=disposition(bounds,self.w0,self.w1)
    for i,role in enumerate(ROLES):
     raw=p.parent/role/'compute-durations.bin'
     self.segment(raw,consumed[i],finish[i],('sealed' if sealed else 'partial',kind,role,window,admission,case),sealed_report['air_page_limit_bounds_role_source_cell_min_max'][i] if sealed else None)
    consumed=finish;begin=None
   for i,role in enumerate(ROLES):
    raw=p.parent/role/'compute-durations.bin';size=raw.stat().st_size
    if sealed:assert sealed_report['role_compute'][i]['raw_bytes']==size
    self.segment(raw,consumed[i],size,('sealed' if sealed else 'partial',kind,role,'unknown_clock_or_incomplete','dangling_or_interrupted',case))
    if sealed:self.check_report(raw,sealed_report['role_compute'][i]['classes'])
 def check_report(self,raw,classes):
  for name in CLASSES:
   r=classes[name];assert r['samples']==self.raw_counts[str(raw)][name],(raw,name,'raw count')
   if r['samples']:assert r['maximum_ns']==self.raw_max[str(raw)][name],(raw,name,'raw maximum')
 def layout(self):
  folder=self.root/'layout'
  if not folder.exists():return
  d=load(self.root/'layout-clock-map.json');driver=load(self.root/'clock-map.json')
  # Subtracting an uncertain driver origin propagates both interval ends.
  origins={(d['pid'],d['instance_unix_ns']):(d['layout_origin_parent_ns_min']-driver['driver_origin_parent_ns_max'],d['layout_origin_parent_ns_max']-driver['driver_origin_parent_ns_min'])}
  raw=folder/'compute-durations.bin';consumed=0
  for row in rows(folder/'events.jsonl'):
   if 'scan' not in row:continue
   assert row['raw_begin']==consumed
   bounds=clock_bounds(row['clock_begin'],row['clock_end'],origins)
   self.segment(raw,consumed,row['raw_end'],('sealed','layout','execution',disposition(bounds,self.w0,self.w1),'scan_complete','L1/full_jprod1'));consumed=row['raw_end']
  compute=load(folder/'report.json')['compute'];assert consumed==raw.stat().st_size==compute['raw_bytes']
  self.check_report(raw,compute['classes'])
 def raw_universe(self):
  universe={str(p):p.stat().st_size for p in self.b.rglob('compute-durations.bin')}
  layout=self.root/'layout/compute-durations.bin'
  if layout.exists():universe[str(layout)]=layout.stat().st_size
  assert set(self.files)<=set(universe)
  for name,size in universe.items():
   if name not in self.files:
    self.unmapped.append(dict(path=name,bytes=size,reason='no bound producer identity or log; retained as unknown, never eligible'))
    self.segment(Path(name),0,size,('unbound','unknown','unknown','unknown_clock_or_incomplete','unbound_identity',name))
   assert self.files[name]==size,('raw universe size mismatch',name)
  assert set(self.files)==set(universe)
 def run(self):
  self.clocks();self.producers();self.layout();self.raw_universe()
  strata=[dict(zip(('record_state','producer','role','window','admission','case','class','page_spec'),k),**statistics(h)) for k,h in sorted(self.hist.items())]
  eligible=[s for s in strata if s['record_state']=='sealed' and s['role']=='execution' and s['window']=='inside' and s['admission'] in ('confirmed_applied','accepted_request_last_attempt','scan_complete') and not s['case'].startswith('unproven_case') and s['page_spec']!='unproven_air_spec']
  expected={'l0':CLASSES[:2],'l1':CLASSES[8:],'l2':CLASSES[2:]}[self.report['scenario']]
  enough=all(any(s['class']==c and s['minimum_samples_met'] for s in eligible) for c in expected)
  passed=not self.unmapped and enough and all(s['thresholds_met'] is True for s in eligible if s['class'] in expected)
  return dict(schema='mycity-steady-window-raw-audit-v1',source_sha256=self.source,formal=self.report['formal'],measurement_driver_ns=[self.w0,self.w1],clock_mappings=len(self.origins),worker_request_intervals=len(self.workers),raw_files_accounted=len(self.files),raw_universe_fully_accounted=True,unbound_raw_files=self.unmapped,raw_bytes_accounted=sum(self.files.values()),batch_intervals=[dict(key=list(k),count=v) for k,v in sorted(self.batches.items())],strata=strata,expected_classes=expected,eligible_window_class_minimums_met=enough,observed_window_thresholds_met=passed,formal_latency='not_accepted_smoke' if not self.report['formal'] else 'requires_same_source_rare_strata_and_final_review',three_rounds='not_accepted_by_single_round',resource_and_queue='not_assessed_by_raw_selector',eight_hour='not_run',real_16GB='not_run',full_M0='not_accepted')
if __name__=='__main__':
 print(json.dumps(Audit(Path(sys.argv[1])).run(),indent=2))
