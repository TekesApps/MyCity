#!/usr/bin/env python3
"""Functional cold-read probe on a copied failed world; no advance/performance claim."""
import hashlib,json,os,selectors,shutil,subprocess,time
from pathlib import Path
D=Path(__file__).resolve().parent;R=D.parents[2];source=R/'work/t13a3d-slots-01/model';dest=R/'work/t13a3d-partial-recovery-01';dest.mkdir();world=dest/'world'
sha=lambda b:hashlib.sha256(b).hexdigest()
manifest=json.loads((D/'t13a3d-slots-01/manifest.json').read_text())
rows=[v for v in manifest if v['path'].startswith('model/world/')]
for v in rows:assert sha((R/'work/t13a3d-slots-01'/v['path']).read_bytes())==v['sha256']
skip={'LOCK','air-worker.lock','air-worker-owner.json','air-worker'}
shutil.copytree(source/'world',world,ignore=lambda directory,names:skip.intersection(names) if Path(directory)==source/'world' else set())
for p in world.rglob('*'):
 assert not p.is_symlink()
 if p.is_file():
  with p.open('rb') as f:os.fsync(f.fileno())
# The original transcript binds the first original ReceiptProof payload hash.
expected=None;request=None
for line in (source/'transcript.jsonl').read_text().splitlines():
 row=json.loads(line)['data']
 if row.get('command',{}).get('kind')=='receipt_proof' and row['command']['operation']==1:request=row['request']
 if request and row.get('response')==request and row['kind']=='result':expected=row['payload_sha256'];break
assert expected
exe=R/'target/release/mycity-headless';env=os.environ.copy();env.pop('MYCITY_DIAGNOSTICS',None);env.pop('MYCITY_HEAP_LOG',None)
log=[];pending=bytearray()
with (dest/'stderr.txt').open('wb') as stderr:
 child=subprocess.Popen([str(exe),'sim-host-scale-full','open',str(world)],stdin=subprocess.PIPE,stdout=subprocess.PIPE,stderr=stderr,env=env)
 selector=selectors.DefaultSelector();selector.register(child.stdout,selectors.EVENT_READ)
 def receive():
  deadline=time.monotonic()+30
  while b'\n' not in pending:
   if time.monotonic()>deadline:raise RuntimeError('cold read probe timeout, not a performance gate')
   if not selector.select(min(1,max(0,deadline-time.monotonic()))):continue
   b=os.read(child.stdout.fileno(),65536)
   if not b:raise RuntimeError('sim EOF')
   pending.extend(b)
   if len(pending)>65536:raise RuntimeError('control frame bound')
  line,_,tail=pending.partition(b'\n');pending[:]=tail;v=json.loads(line);log.append(v);return v
 def call(i,command):
  child.stdin.write((json.dumps({'protocol':'mycity-sim-v1','id':i,'command':command},separators=(',',':'))+'\n').encode());child.stdin.flush()
  v=receive();assert v['id']==i and v['kind']=='result' and 'Ok' in v['payload'];return v['payload']
 try:
  ready=receive();assert ready['id']==0 and ready['kind']=='ready'
  before=call(1,{'kind':'progress'})['Ok'];proof=call(2,{'kind':'receipt_proof','operation':1})
  assert sha(json.dumps(proof,sort_keys=True,separators=(',',':'),ensure_ascii=False).encode())==expected and proof['Ok']['present']
  after=call(3,{'kind':'progress'})['Ok'];assert before==after
  call(4,{'kind':'stop'});child.stdin.close();assert child.wait(timeout=30)==0
 finally:
  selector.close()
  if child.poll() is None:child.kill();child.wait()
for v in rows:assert sha((R/'work/t13a3d-slots-01'/v['path']).read_bytes())==v['sha256'],'original failure mutated'
(dest/'events.jsonl').write_text(''.join(json.dumps(v,separators=(',',':'))+'\n' for v in log))
result={'status':'copied_partial_world_cold_read_passed','source_original_files_unchanged':len(rows),'excluded_disposable_runtime_names':sorted(skip),'recovered_sequence':before['sequence'],'oldest_original_receipt_proof_hash_equal':True,'observation_changes_no_progress':True,'commands':['progress','receipt_proof(1)','progress','stop'],'game_advance_performed':False,'scope':'functional cold read on copy only; no 30day completion, perf, pressure or 8h pass'}
(dest/'report.json').write_text(json.dumps(result,indent=2)+'\n');print(json.dumps(result,indent=2))
