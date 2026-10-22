from pathlib import Path
import os,json,hashlib,subprocess,select,time,signal
base=Path('/Volumes/HP P900/dev/MyCity/work/j3f-01/observed');src=base/'model/business/world'
root=Path('/Volumes/HP P900/dev/MyCity/work/formal01-poststop-business-01');root.mkdir(exist_ok=False); dst=root/'world';dst.mkdir()
exe=Path('/Volumes/HP P900/dev/MyCity/work/candidate-binaries/r07-extents/32c6e61b9565545177c330f34455611b329fbe1cb38591ec5f6e7879731c4da2/mycity-headless')
assert hashlib.sha256(exe.read_bytes()).hexdigest()=='59aa365ed46b5d373218fcbcf1d34f930d674d15607ed510190361891439b4e0'
def manifest(path):
 rows=[]
 for f in sorted(path.rglob('*')):
  assert not f.is_symlink(),str(f)
  if f.is_dir():continue
  assert f.is_file(),str(f)
  rows.append({'path':str(f.relative_to(path)),'size':f.stat().st_size,'sha256':hashlib.sha256(f.read_bytes()).hexdigest()})
 return rows
before=manifest(src)
for row in before:
 a=src/row['path'];b=dst/row['path'];b.parent.mkdir(parents=True,exist_ok=True)
 subprocess.run(['/bin/cp','-c',str(a),str(b)],check=True)
assert manifest(dst)==before and manifest(src)==before
(root/'original-manifest.json').write_text(json.dumps(before,indent=2))
expected=None; proof=None
with (base/'model/business/transcript.jsonl').open() as f:
 for line in f:
  r=json.loads(line);d=r.get('data',{})
  if 'endurance_verified_target' in d:expected=d['state']
  if d.get('endurance_fault')=='sim_committed_reply':proof=d['original_proof']
assert expected and proof
(root/'expected-state.json').write_text(json.dumps(expected,indent=2));(root/'expected-receipt.json').write_text(json.dumps(proof,indent=2))
e=os.environ.copy()
for k in tuple(e):
 if k.startswith('MYCITY_'):e.pop(k,None)
keys=['sequence','business_sha256','background','timeline','state_sha256'];results=[]
for attempt in [1,2]:
 d=root/('wal-cold' if attempt==1 else 'checkpoint-cold');d.mkdir()
 with (d/'stderr.log').open('wb') as stderr,(d/'protocol.jsonl').open('w') as log:
  p=subprocess.Popen([str(exe),'sim-host-scale-full-probe','open',str(dst)],stdin=subprocess.PIPE,stdout=subprocess.PIPE,stderr=stderr,env=e,start_new_session=True,bufsize=0)
  buf=bytearray();next_id=0
  def read():
   until=time.monotonic()+30
   while b'\n' not in buf:
    wait=until-time.monotonic()
    if wait<=0 or not select.select([p.stdout],[],[],max(0,wait))[0]:raise TimeoutError('sim response timeout')
    b=os.read(p.stdout.fileno(),65536)
    if not b:raise RuntimeError('sim EOF, exit='+str(p.poll()))
    buf.extend(b)
    if len(buf)>65536:raise RuntimeError('sim frame exceeds fixed 64KiB')
   n=buf.index(10);raw=bytes(buf[:n]);del buf[:n+1];value=json.loads(raw)
   log.write(json.dumps({'response':value})+'\n');log.flush();return value
  def call(command):
   global next_id
   next_id+=1;r={'protocol':'mycity-sim-v1','id':next_id,'command':command}
   log.write(json.dumps({'request':r})+'\n');log.flush();p.stdin.write(json.dumps(r,separators=(',',':')).encode()+b'\n');p.stdin.flush()
   reply=read();assert reply['protocol']=='mycity-sim-v1' and reply['id']==next_id and reply['kind']=='result'
   payload=reply['payload'];assert 'Ok' in payload,payload;return payload['Ok']
  try:
   ready=read();assert ready['kind']=='ready' and ready['id']==0
   actual=call({'kind':'status'});matched={k:actual[k]==expected[k] for k in keys}
   assert actual['source_sha256']=='32c6e61b9565545177c330f34455611b329fbe1cb38591ec5f6e7879731c4da2'
   current_proof=call({'kind':'receipt_proof','operation':proof['operation']})
   item={'attempt':attempt,'status_matches_last_committed_verification':matched,'known_receipt_equal':current_proof==proof,'scope':'standalone cold restore; not shared-pool or 6h/8h performance acceptance'}
   (d/'status.json').write_text(json.dumps(actual,indent=2));(d/'receipt-proof.json').write_text(json.dumps(current_proof,indent=2))
   if attempt==1:
    call({'kind':'checkpoint'});after=call({'kind':'status'});item['checkpoint_preserves_state']={k:after[k]==actual[k] for k in keys}
   call({'kind':'stop'});p.stdin.close();rc=p.wait(timeout=10);item['exit_code']=rc;assert rc==0
   results.append(item)
  finally:
   if p.poll() is None:
    os.killpg(p.pid,signal.SIGTERM)
    try:p.wait(timeout=10)
    except subprocess.TimeoutExpired:os.killpg(p.pid,signal.SIGKILL);p.wait()
unchanged=manifest(src)==before
receipt={'results':results,'original_full_files_unchanged':unchanged,'source_original':str(src),'full_copy':str(dst),'memory_16gb_gate':'not_run','eight_hour_gate':'not_run_incomplete_attempt'}
(root/'receipt.json').write_text(json.dumps(receipt,indent=2));print(json.dumps(receipt))
assert unchanged and all(all(x['status_matches_last_committed_verification'].values()) and x['known_receipt_equal'] and x['exit_code']==0 for x in results)
