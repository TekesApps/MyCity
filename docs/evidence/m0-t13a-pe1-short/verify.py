#!/usr/bin/env python3
"""Independent original WAL + raw timing audit, never trusts report counts."""
import collections,hashlib,json,math,struct,sys,zipfile
from pathlib import Path
D=Path(__file__).resolve().parent
root=Path(sys.argv[1]);reference=json.loads((D/'reference-worlds.json').read_text())
keys={'debt':'debt_exit','prepay':'prepaid_cancel','refund':'refund_shortfall','suspend':'suspended_shift'}
expected={'debt':4,'prepay':26,'refund':27,'suspend':49}
plan=json.loads((root/'run-plan.json').read_text());report=json.loads((root/'report.json').read_text())
counts=collections.Counter();active=collections.defaultdict(list);excluded=collections.Counter();successes=0;failures=0;rawbytes=0;logbytes=0
for row in report['trials']:
 kind=row['case'];p=root/'archives'/(row['trial']+'.zip');assert hashlib.sha256(p.read_bytes()).hexdigest()==row['archive_sha256']
 with zipfile.ZipFile(p) as z:
  manifest=json.loads(z.read('archive-manifest.json'));assert set(z.namelist())=={v['path'] for v in manifest}|{'archive-manifest.json'}
  for f in manifest:
   b=z.read(f['path']);assert len(b)==f['bytes'] and hashlib.sha256(b).hexdigest()==f['sha256']
  r=json.loads(z.read('cli.stdout'));assert r['report']['world']==reference[keys[kind]]
  cur=json.loads(z.read('world/CURRENT.json'));b=z.read(f"world/root-{cur['slot']}.json");assert hashlib.sha256(b).hexdigest()==cur['root_sha256'];authority=json.loads(b)
  genesis=z.read('world/genesis.json');assert hashlib.sha256(genesis).hexdigest()==authority['genesis_sha256']
  wal=z.read('world/wal.bin');assert len(wal)==authority['wal_end'];pos=0;previous=authority['genesis_sha256'];receipts={};sequence=0
  while pos<len(wal):
   length=struct.unpack_from('<I',wal,pos)[0];end=pos+4+length;checksum=hashlib.sha256(wal[pos:end]).digest();assert checksum==wal[end:end+32]
   w=json.loads(wal[pos+4:end]);sequence+=1;assert w['sequence']==sequence and w['previous']==previous
   op=w['envelope']['operation_id'];assert op==sequence and op not in receipts;receipts[op]=w;previous=checksum.hex();pos=end+32
  assert previous==authority['wal_chain'] and sequence==authority['sequence']==r['report']['committed_sequence']
  failed=[op for op,w in receipts.items() if 'Err' in w['record']['outcome']];assert failed==([3] if kind=='debt' else [])
  if failed:assert receipts[3]['envelope']['actions'][0]['command']['kind']=='close'
  failures+=len(failed);successes+=len(receipts)-len(failed)
  noops=set()
  if kind in ['prepay','refund']:
   assert r['report']['world']['economy']['last_result']=='AlreadyCanceled'
   assert receipts[sequence]['envelope']['actions']==receipts[sequence-1]['envelope']['actions']==[{'kind':'economy','command':{'kind':'cancel_future'}}]
   noops={sequence}
  name=next(n for n in z.namelist() if n.endswith('/transactions.jsonl'));prefix=name.removesuffix('transactions.jsonl')
  raw=z.read(prefix+'execution/compute-durations.bin');assert len(raw)%9==0;rawbytes+=len(raw)
  log=json.loads(z.read(prefix+'report.json'));assert log['source_sha256']==row['source_sha256']
  assert z.read(prefix+'quote/compute-durations.bin')==z.read(prefix+'replay/compute-durations.bin')==b''
  logs=[json.loads(l) for l in z.read(name).splitlines()];begin=None;offset=0;seen=set();got=0
  for line in logs:
   if line.get('phase')=='begin':begin=line;continue
   if line.get('kind')!='transaction':continue
   op=line['operation'];assert op not in seen;seen.add(op);assert op in receipts and 'Ok' in receipts[op]['record']['outcome']
   assert line['durable_receipt'] and line['summary']['applied'] and line['summary']['hook_succeeded'] and not line['summary']['replayed']
   assert begin['identity']['operation']==op and begin['raw_offsets']==[offset,0,0]
   c,ns=struct.unpack_from('<BQ',raw,offset);assert c==1
   roles=line['summary']['roles'][0];assert ns==roles[0]['nanos']+roles[1]['nanos']
   offset+=9;assert line['raw_offsets_end']==[offset,0,0]
   if op in noops:excluded[kind]+=1
   else:active[kind].append(ns);got+=1
  assert offset==len(raw) and seen==set(receipts)-set(failed) and got==expected[kind]
  counts[kind]+=1
  proof=json.loads((root/(row['trial']+'-cold.stdout')).read_text())
  for k in ['world','state_sha256','committed_sequence']:assert proof[k]==r['report'][k]
  logbytes+=row['sizes']['log']
assert counts==plan['counts'];stats={}
for kind,values in active.items():
 values.sort();n=len(values);p95=values[math.ceil(n*.95)-1];p99=values[math.ceil(n*.99)-1]
 stats[kind]={'samples':n,'p95_ns':p95,'p99_ns':p99,'max_ns':values[-1],'sample_minimum_met':n>=200,'latency_20_40ms_met':p95<=20000000 and p99<=40000000}
result={'status':'original_WAL_raw_intervals_complete_World_and_cold_proofs_verified','counts':dict(counts),'successful_transactions':successes,'failed_Close_receipts':failures,'excluded_AlreadyCanceled':dict(excluded),'active_case_compute':stats,'raw_bytes':rawbytes,'classified_log_bytes':logbytes,'scope':'finite independent repetitions, not steady/joint/8h/16GB'}
print(json.dumps(result,indent=2))
