import hashlib,json,select,socket,subprocess,time
from pathlib import Path
R=Path('/Users/frankqin/dev/MyCity'); W=R/'work/t13a3h-prefix-before';W.mkdir();P=W/'pool';B=R/'target/release/mycity-headless'
c=subprocess.Popen([str(B),'normal-pool-host',str(P)],stdin=subprocess.PIPE,stdout=(W/'stdout').open('wb'),stderr=(W/'stderr').open('wb'))
try:
 end=time.monotonic()+10
 while not (P/'ready.json').exists():
  assert c.poll() is None and time.monotonic()<end;time.sleep(.01)
 r=json.loads((P/'ready.json').read_text());assert r['pid']==c.pid
 packet=json.dumps({'source':r['source'],'boot':r['boot'],'action':{'attach':{'id':0,'key':'0'*64,'pid':1}}},separators=(',',':')).encode()+b'\n'
 with socket.socket(socket.AF_UNIX) as s:
  s.settimeout(5);s.connect(str(P/'pool.sock'));start=time.monotonic();s.sendall(packet[:17]);early=bool(select.select([s],[],[],.05)[0]);elapsed=time.monotonic()-start
  if not early:s.sendall(packet[17:])
  reply=b''
  while not reply.endswith(b'\n'):
   chunk=s.recv(8192)
   if not chunk:break
   reply+=chunk
 result={'scope':'synthetic fragmented transport; fake unauthorized grant; no business','source':r['source'],'binary_sha256':hashlib.sha256(B.read_bytes()).hexdigest(),'packet':packet.decode(),'reply_before_suffix':early,'prefix_wait_seconds':elapsed,'reply':json.loads(reply)}
finally:c.stdin.close();code=c.wait(timeout=10)
result['pool_exit']=code;(W/'result.json').write_text(json.dumps(result,indent=2)+'\n');print(json.dumps(result,indent=2))
