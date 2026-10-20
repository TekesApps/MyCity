#!/usr/bin/env python3
"""Supplemental read-only causal witness; outside original measured process tree."""
import hashlib,importlib.util,json,os,sys,time
from pathlib import Path
D=Path(__file__).resolve().parent;root=Path(sys.argv[1]);pending=root/'model/layout/snapshots/snapshot-000002.pending';world=root/'model/business/world';transcript=root/'model/business/transcript.jsonl'
spec=importlib.util.spec_from_file_location('observer_api',D/'observe_v3.py');api=importlib.util.module_from_spec(spec);spec.loader.exec_module(api)
limit=time.monotonic()+300
while not pending.exists():
 if time.monotonic()>limit:raise RuntimeError('second snapshot not observed before witness deadline')
 time.sleep(.05)
def authority():
 for _ in range(100):
  before=pending.exists()
  cb=(world/'CURRENT.json').read_bytes();c=json.loads(cb);rb=(world/f"root-{c['slot']}.json").read_bytes()
  if hashlib.sha256(rb).hexdigest()==c['root_sha256'] and pending.exists() and before:
   return {'observed_monotonic_seconds':time.monotonic(),'pending_snapshot_observed_before_and_after_read':True,'current_bytes':cb.decode(),'root_bytes':rb.decode(),'sequence':json.loads(rb)['sequence'],'witness_usage':api.usage(os.getpid())}
  time.sleep(.001)
 raise RuntimeError('no stable root during snapshot')
first=authority();offset=transcript.stat().st_size;offset_end=offset;lines=[];tail=b'';last=None
while pending.exists():
 with transcript.open('rb') as f:f.seek(offset_end);b=f.read(65536)
 offset_end+=len(b);tail+=b
 while b'\n' in tail:
  line,tail=tail.split(b'\n',1)
  try:v=json.loads(line)
  except json.JSONDecodeError:continue # starting offset may land in a partial producer line
  if 'verified_target' in v.get('data',{}):lines.append({'sha256':hashlib.sha256(line+b'\n').hexdigest(),'line':v})
 if lines:
  candidate=authority()
  if candidate['sequence']>first['sequence']:last=candidate;break
 time.sleep(.05)
result={'schema':'mycity-readonly-snapshot-overlap-witness-v1','first':first,'last':last,'complete_targets_observed':lines,'transcript_byte_range':[offset,offset_end],'witness_source_sha256':hashlib.sha256(Path(__file__).read_bytes()).hexdigest(),'scope':'supplemental functional overlap; witness is outside original measured process tree and group performance budget remains not accepted'}
with (root/'overlap-witness.json').open('x') as f:json.dump(result,f,indent=2);f.write('\n');f.flush();os.fsync(f.fileno())
assert last is not None and lines,'no actual durable progress witnessed during snapshot'
print(json.dumps({'sequence_before':first['sequence'],'sequence_after':last['sequence'],'complete_targets':len(lines)}))
