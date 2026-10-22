"""One bounded startup exchange; integer causal bounds, never a cross-process epoch guess."""
import json,os,time
from pathlib import Path
SCHEMA='mycity-observer-clock-reply-v1'
def bounds(reply,pid,source,nonce,sent,received,spawn_before):
 if reply.get('schema')!=SCHEMA or reply.get('pid')!=pid or reply.get('source_sha256')!=source or reply.get('nonce')!=nonce:
  raise RuntimeError('clock reply identity mismatch')
 d=reply.get('elapsed_ns')
 if type(d) is not int or d<0 or type(sent) is not int or type(received) is not int or spawn_before>sent or sent>received:
  raise RuntimeError('clock numeric bounds')
 lo=max(spawn_before,sent-d);hi=received-d
 if lo>hi:raise RuntimeError('clock causal intersection empty')
 return lo,hi

def wait_json(child,path,deadline):
 while True:
  if child.poll() is not None:raise RuntimeError('clock peer exited')
  if time.monotonic()>=deadline:raise RuntimeError('clock exchange timeout')
  try:
   with path.open('rb') as f:data=f.read(2049)
  except FileNotFoundError:data=b''
  if len(data)>2048:raise RuntimeError('clock message size')
  if data.endswith(b'\n'):return json.loads(data)
  time.sleep(.001)

def exchange(child,root,start_ns,spawn_before,expected_source):
 model=Path(root)/'model';deadline=time.monotonic()+10
 ready=wait_json(child,model/'observer-clock-ready.json',deadline)
 if ready!={'schema':'mycity-observer-clock-ready-v1','pid':child.pid,'source_sha256':expected_source}:
  raise RuntimeError('clock ready identity mismatch')
 nonce=os.urandom(16).hex();request={'schema':'mycity-observer-clock-request-v1','pid':child.pid,'source_sha256':expected_source,'nonce':nonce}
 sent=time.monotonic_ns()-start_ns
 temporary=model/'observer-clock-request.tmp'
 with temporary.open('x') as f:
  json.dump(request,f);f.write('\n');f.flush();os.fsync(f.fileno())
 os.rename(temporary,model/'observer-clock-request.json')
 reply=wait_json(child,model/'observer-clock-reply.json',deadline)
 received=time.monotonic_ns()-start_ns
 lo,hi=bounds(reply,child.pid,expected_source,nonce,sent,received,spawn_before)
 result={'supervisor_pid':child.pid,'source_sha256':expected_source,'supervisor_origin_observer_ns_min':lo,'supervisor_origin_observer_ns_max':hi,'uncertainty_ns':hi-lo,'method':'nonce-bound real request/reply; observer send <= supervisor timestamp <= observer receipt; intersect original spawn lower bound','request_sent_observer_ns':sent,'reply_received_observer_ns':received,'spawn_before_observer_ns':spawn_before,'nonce':nonce,'reply':reply,'raw_origin_bounds_ns':[sent-reply['elapsed_ns'],received-reply['elapsed_ns']]}
 (Path(root)/'observer-clock-map.json').write_text(json.dumps(result,indent=2)+'\n')
 return result
