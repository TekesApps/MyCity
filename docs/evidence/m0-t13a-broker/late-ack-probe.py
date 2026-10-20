import hashlib,json,socket,subprocess,sys,time
from pathlib import Path
R=Path('/Users/frankqin/dev/MyCity');W=R/'work'/sys.argv[1];W.mkdir();P=W/'pool';B=R/'target/release/mycity-headless'
c=subprocess.Popen([str(B),'normal-pool-host',str(P)],stdin=subprocess.PIPE,stdout=(W/'stdout').open('wb'),stderr=(W/'stderr').open('wb'))
try:
 end=time.monotonic()+10
 while True:
  assert c.poll() is None and time.monotonic()<end
  try:r=json.loads((P/'ready.json').read_text())
  except (FileNotFoundError,json.JSONDecodeError):time.sleep(.005);continue
  assert r['pid']==c.pid;break
 def rpc(op):
  with socket.socket(socket.AF_UNIX) as s:
   s.settimeout(5);s.connect(str(P/'pool.sock'));s.sendall(json.dumps({'source':r['source'],'boot':r['boot'],'action':{'broker':op}},separators=(',',':')).encode()+b'\n');b=b''
   while not b.endswith(b'\n'):
    p=s.recv(8192)
    if not p:raise RuntimeError('eof')
    b+=p
  result=json.loads(b);assert result['error'] is None;return result['broker']
 # Deliberately fixed synthetic supervisor keys; no Sim or worker is spawned.
 first=rpc({'register':{'supervisor':'1'*64}})['registered']['generation']
 rpc({'cancel':{'generation':first,'key':'1'*64}})
 second=rpc({'register':{'supervisor':'2'*64}})['registered']['generation']
 before=(P/'events.jsonl').read_text().splitlines()
 response=rpc({'acknowledge_revoke':{'generation':first,'key':'1'*64}})
 after=(P/'events.jsonl').read_text().splitlines()
 appended=[{'kind':v['kind'],'broker_generation':v['broker_generation']} for v in map(json.loads,after[len(before):])]
 rpc({'cancel':{'generation':second,'key':'2'*64}})
 result={'scope':'synthetic control only, fixed fake supervisor keys, no Sim/worker/business','source_sha256':r['source'],'binary_sha256':hashlib.sha256(B.read_bytes()).hexdigest(),'old_generation':first,'new_generation':second,'late_ack_receipt_generation':response['revoked']['generation'],'appended_events':appended,'wrong_generation_event':any(v['broker_generation']!=first for v in appended)}
finally:c.stdin.close();code=c.wait(timeout=10)
result['pool_exit']=code;(W/'result.json').write_text(json.dumps(result,indent=2)+'\n');print(json.dumps(result,indent=2))
