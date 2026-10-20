#!/usr/bin/env python3
"""Wall intervals of persistent boundaries; not pure computation or readiness gates."""
import json,math,collections,zipfile
from pathlib import Path
D=Path(__file__).resolve().parent
spans={'wal_encode_hash':(0,1),'capacity_and_continuation_admit':(1,2),'wal_write':(2,6),'wal_fsync':(6,7),'root_construct_encode':(7,8),'root_atomic_publish_sync':(8,9),'current_hash_encode':(9,10),'current_write_rename':(10,11),'current_directory_sync':(11,12),'post_persist_apply':(12,13)}
def stats(a):
 s=sorted(a);return {'samples':len(s),'sum_ns':sum(s),'p95_ns':s[math.ceil(len(s)*.95)-1],'p99_ns':s[math.ceil(len(s)*.99)-1],'max_ns':s[-1]}
result={}
for folder in D.glob('t13a2e-*'):
 if not (folder/'artifacts.zip').exists():continue
 groups=collections.defaultdict(list);unconfirmed=0
 with zipfile.ZipFile(folder/'artifacts.zip') as z:
  for name in z.namelist():
   if not name.endswith('/transactions.jsonl'):continue
   for b in z.read(name).splitlines():
    v=json.loads(b)
    if v.get('phase')!='persistence_boundaries':continue
    if v['commit_status']!='confirmed':unconfirmed+=1;continue
    points=v['points_ns']
    present=[p for p in points if p is not None];assert all(a<=b for a,b in zip(present,present[1:]))
    for phase,(lo,hi) in spans.items():
     if points[lo] is not None and points[hi] is not None:groups[phase].append(points[hi]-points[lo])
 result[folder.name]={'confirmed_persistence_intervals':{k:stats(v) for k,v in groups.items()},'unconfirmed_requires_reopen_receipt_query':unconfirmed}
(D/'io-summary.json').write_text(json.dumps({'scope':'fixed observer timestamps; elapsed includes callback overhead, preemption and I/O; not compute gate', 'runs':result},indent=2)+'\n')
print(json.dumps(result,indent=2))
