"""Exact input/output identity and descriptive same-anchor timings, not formal quantiles."""
import json
from pathlib import Path
D=Path(__file__).resolve().parent
phases=['baseline','candidate','software','hardware'];out=[]
for kind in ['source','claims']:
 runs={p:json.loads((D/(p+'-'+kind+'.json')).read_text()) for p in phases};reference=runs['baseline'];identity=['image_sha256','air_sha256','air_bytes','source_count','last_complete','target','quoted']
 for r in runs.values():
  assert r['warmup']==5 and r['samples']==40
  assert all(reference[k]==r[k] for k in identity)
  for row in r['rows']:assert len(row['raw_ns'])==40 and row['p50_ns']==sorted(row['raw_ns'])[19] and row['p95_ns']==sorted(row['raw_ns'])[37] and row['p99_ns']==sorted(row['raw_ns'])[39]
  for index in range(3):
   for a,b in zip(reference['trace']['roles'][index],r['trace']['roles'][index]):
    assert all(a[k]==b[k] for k in ['calls','work','paid','scanned'])
 metrics={p:{r['name']:{k:r[k] for k in ['p50_ns','p95_ns','p99_ns']} for r in x['rows']} for p,x in runs.items()}
 out.append({'anchor':kind,'identities':{k:reference[k] for k in identity},'sources':{p:r['source'] for p,r in runs.items()},'metrics':metrics})
print(json.dumps({'scope':'40-repeat offline descriptive diagnostic; canonical+software and hardware changes separated; no formal or simultaneous-load acceptance','cases':out,'known_limitations':'sequential executions; elapsed wall including preemption and return drop; not a controlled thermal/environment causal speedup claim'},indent=2))
