#!/usr/bin/env python3
"""Independent exact raw timing audit, offline only. Never promotes smoke to a formal gate."""
import collections,json,math,struct,sys
from pathlib import Path
D=Path(__file__).resolve().parent;R=Path(sys.argv[1]);B=R/'model/business';load=lambda p:json.loads(p.read_text())
F=load(D.parents[2]/'fixtures/m0-t13a-acceptance.json');P=F['latency'];classes=P['compute_classes'];assert len(classes)==10
source=load(D/'source.json')['source_sha256'];roles=['execution','quote','replay'];hist=collections.defaultdict(collections.Counter);raw_bytes=0;sealed=0;partial=0
for d in sorted((B/'diagnostics').iterdir()):
 if not d.is_dir():continue
 report=load(d/'report.json') if (d/'report.json').exists() else None
 kind=d.name.split('-')[0];assert kind in ['sim','worker']
 status='sealed' if report else 'partial_fault_preserved'
 if report:assert report['source_sha256']==source and report['producer']==kind;sealed+=1
 else:partial+=1
 for role_index,role in enumerate(roles):
  p=d/role/'compute-durations.bin';counts=collections.Counter();maxima=collections.defaultdict(int);nbytes=0
  with p.open('rb') as f:
   while b:=f.read(9*8192):
    assert len(b)%9==0,'truncated raw event';nbytes+=len(b);raw_bytes+=len(b);assert raw_bytes<=128*1024**2
    for c,ns in struct.iter_unpack('<BQ',b):
     assert c<len(classes);name=classes[c];counts[name]+=1;maxima[name]=max(maxima[name],ns)
     h=hist[(status,kind,role,name)];h[ns]+=1;assert len(h)<=1000000,'offline histogram distinct-value guard'
  if report:
   rr=report['role_compute'][role_index];assert rr['raw_bytes']==nbytes
   for name in classes:
    r=rr['classes'][name];assert r['samples']==counts[name]
    if counts[name]:assert r['maximum_ns']==maxima[name]
def stats(h):
 n=sum(h.values());ordered=sorted(h.items())
 def q(percent):
  if n==0:return None
  rank=(n*percent+99)//100;seen=0
  for ns,count in ordered:
   seen+=count
   if seen>=rank:return ns
 p95=q(95);p99=q(99);enough=n>=P['minimum_class_samples_for_quantiles']
 return {'samples':n,'maximum_ns':max(h) if n else None,'p95_ns':p95,'p99_ns':p99,'minimum_samples_met':enough,'observed_p95_p99_thresholds_met':None if not enough else p95<=P['batch_compute_p95_us']*1000 and p99<=P['batch_compute_p99_us']*1000}
strata=[{'record_state':status,'producer':kind,'role':role,'class':name,**stats(h)} for (status,kind,role,name),h in sorted(hist.items())]
missing=[name for name in classes if not any(s['record_state']=='sealed' and s['role']=='execution' and s['class']==name for s in strata)]
fail=[s for s in strata if s['record_state']=='sealed' and s['role']=='execution' and s['observed_p95_p99_thresholds_met'] is False]
phase_hist=collections.defaultdict(collections.Counter);region_hist=collections.defaultdict(collections.Counter)
for d in sorted((B/'diagnostics').glob('sim-*')):
 if not (d/'report.json').exists():continue
 r=load(d/'report.json');names=r['classes'];ix={n:names.index(n) for n in ['capture','admit','region']}
 for line in (d/'transactions.jsonl').open():
  row=json.loads(line);sm=row.get('summary')
  if not sm or not row.get('durable_receipt') or not sm['applied'] or not sm['hook_succeeded'] or sm['replayed']:continue
  counters=sm['roles'][0]
  for name in ['capture','admit']:
   c=counters[ix[name]]
   if c['work']>0:phase_hist[name][c['nanos']]+=1
  c=counters[ix['region']]
  if c['work']>0:
   kinds=tuple(n for n,count in zip(r['region_subtypes'],sm['region_subtypes'][0]) if count)
   region_hist[kinds][c['nanos']]+=1
assert sum(sum(h.values()) for h in phase_hist.values())==sum(hist[('sealed','sim','execution','L2_background_capture_admit_cpu')].values())
out={'source_sha256':source,'scope':'same-source raw full-smoke timing diagnostics; offline analysis; concurrent regression; not performance acceptance','raw_bytes':raw_bytes,'sealed_producers':sealed,'partial_producers':partial,'quantile':'exact nearest rank on raw nanoseconds; no pooling across producer/role/sealed state','minimum_class_samples':P['minimum_class_samples_for_quantiles'],'p95_limit_ns':P['batch_compute_p95_us']*1000,'p99_limit_ns':P['batch_compute_p99_us']*1000,'strata':strata,'background_subregion_diagnostics':{n:stats(h) for n,h in phase_hist.items()},'region_subtype_diagnostics':{' + '.join(k):stats(h) for k,h in region_hist.items()},'region_overlap':'Region P0 includes capture; do not add region and capture durations','missing_sealed_execution_classes':missing,'execution_strata_exceeding_observed_thresholds':fail,'formal_10class_gate':'not_accepted; missing L0/L1/rare strata and formal same-source joint protocol','four_speeds':'not_run','steady':'not_run','eight_hour':'not_run','real_16GB':'not_run','full_M0':'not_accepted'}
print(json.dumps(out,indent=2))
