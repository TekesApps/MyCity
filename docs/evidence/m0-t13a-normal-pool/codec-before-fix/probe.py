import json,subprocess,socket,time,random,os
from pathlib import Path
R=Path('/Users/frankqin/dev/MyCity');W=R/'work/t13a3h-codec-probe';W.mkdir();P=W/'pool';B=R/'target/release/mycity-headless';events=[];rng=random.Random(73)
child=subprocess.Popen([str(B),'normal-pool-host',str(P)],stdin=subprocess.PIPE,stdout=(W/'stdout').open('wb'),stderr=(W/'stderr').open('wb'))
try:
 deadline=time.monotonic()+10
 while not (P/'ready.json').exists():
  assert child.poll() is None
  assert time.monotonic()<deadline
  time.sleep(.01)
 ready=json.loads((P/'ready.json').read_text());assert ready['pid']==child.pid
 for i in range(2000):
  key='0'*64
  variants=[{'attach':{'id':0,'key':key,'pid':i+1}},{'filled':{'id':0,'key':key,'identity':{'process_pid':i+1,'process_instance_unix_ns':1792521804324903000+i*1000,'run_epoch':i+1,'owner_instance':0,'job':i+1,'generation':1,'attempt':1},'capacity_bytes':4194304,'available_touched_bytes':4194304,'full_balance_scan_passed':True}},{'reap':{'id':0,'key':key,'pid':i+1,'exit_success':bool(i%2)}},{'abort':{'id':0,'key':key}}]
  packet=json.dumps({'source':ready['source'],'boot':ready['boot'],'action':variants[i%4]},separators=(',',':')).encode()+b'\n'
  with socket.socket(socket.AF_UNIX) as s:
   s.settimeout(5);s.connect(str(P/'pool.sock'));cursor=0
   while cursor<len(packet):
    end=min(cursor+rng.randrange(1,25),len(packet));s.sendall(packet[cursor:end]);cursor=end
   response=b''
   while not response.endswith(b'\n'):response+=s.recv(8192)
  reply=json.loads(response)
  if reply['error']!='normal_pool_stale_or_wrong_grant':events.append({'i':i,'packet':packet.decode(),'response':reply})
  if len(events)>=5:break
finally:
 child.stdin.close();code=child.wait(timeout=10)
(W/'result.json').write_text(json.dumps({'scope':'synthetic codec only; fake grant id; no real business or worker allocations','requests':i+1,'unexpected':events,'pool_exit':code},indent=2)+'\n');print((W/'result.json').read_text())
