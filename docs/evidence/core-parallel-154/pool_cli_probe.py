from pathlib import Path
import subprocess,json,time,hashlib
b=Path(__file__).resolve().parent
pin=json.loads((b/'pool-diagnostic-fixed-396.json').read_text())
root=Path('/Volumes/HP P900/dev/MyCity/work/pm397')
assert not root.exists(),root
assert hashlib.sha256(Path(pin['binary']).read_bytes()).hexdigest()==pin['binary_sha256']
p=subprocess.Popen([pin['binary'],'normal-pool-host',str(root)],stdin=subprocess.PIPE,stdout=subprocess.PIPE,stderr=subprocess.PIPE)
t=time.monotonic()
try:
 while not (root/'ready.json').exists():
  assert p.poll() is None,'child exited before ready'
  assert time.monotonic()-t<10,'ready timeout'
  time.sleep(.01)
 ready=json.loads((root/'ready.json').read_text());assert ready['pid']==p.pid
 p.stdin.close();p.stdin=None
 out,err=p.communicate(timeout=10)
 assert p.returncode==0,(p.returncode,err.decode())
 report=json.loads((root/'report.json').read_text());assert report['source_sha256']==pin['source_sha256']
 assert report['balanced_after_owned_wait']
 raw=(root/'events.jsonl').read_bytes();lines=raw.splitlines(keepends=True)
 assert lines and all(len(row)<=1536 for row in lines)
 events=[json.loads(row) for row in lines]
 for event in events:
  mem=event['mem_v1'];assert len(mem['heap_bytes'])==2 and mem['heap_bytes'][1]>=mem['heap_bytes'][0]>0
  assert mem['arena_bytes'] is None and mem['arena_failures'] is None
 assert events[0]['kind']=='created' and events[-1]['kind']=='parent_pipe_closed'
 print(json.dumps({'passed':True,'scope':'standalone64MiB_pool_CLI_wire_and_balanced_shutdown_not_original_M0_gate','full_artifacts':str(root),'source_sha256':report['source_sha256'],'binary_sha256':pin['binary_sha256'],'event_rows':len(events),'event_log_bytes':len(raw),'max_actual_row_bytes_including_newline':max(map(len,lines)),'first_memory':events[0]['mem_v1'],'last_memory':events[-1]['mem_v1'],'balanced_after_owned_wait':True,'native_peak':'not_measured','arena':'not_activated_in_standalone_probe','elapsed_seconds':time.monotonic()-t},indent=2))
finally:
 if p.poll() is None:
  if p.stdin is not None:p.stdin.close()
  p.terminate();p.wait(timeout=10)
