#!/usr/bin/env python3
"""Helper-only allocation and boundary tests, not city simulation/performance acceptance."""
import importlib.util,json,os,subprocess,tempfile,tracemalloc,gc
from pathlib import Path
D=Path(__file__).resolve().parent
spec=importlib.util.spec_from_file_location('v2',D/'observe_v2.py');v2=importlib.util.module_from_spec(spec);spec.loader.exec_module(v2)
raw=subprocess.check_output(['/bin/ps','-axo','pid=,ppid=,rss=,stat=,comm=']);old={}
for line in raw.splitlines():
 p=line.split(None,4);pid,ppid,rss=map(int,p[:3]);old[pid]=(ppid,rss*1024,p[3].decode(),p[4].decode(errors='replace'))
assert v2.parse_ps(raw)==old
with tempfile.TemporaryDirectory(prefix='mycity-observer-helper-') as name:
 root=Path(name)
 for worker in range(724):
  p=root/f'worker-{worker}';p.mkdir()
  for role in ['execution','quote','replay']:
   q=p/role;q.mkdir();(q/'compute-durations.bin').write_bytes(b'123456789' if role=='execution' else b'')
  (p/'report.json').write_bytes(b'{}');(p/'transactions.jsonl').write_bytes(b'{}\n')
 total=sum(p.stat().st_size for p in root.rglob('*') if p.is_file());latency=sum(p.stat().st_size for p in root.glob('**/compute-durations.bin'))
 assert v2.disk_scan(root)==(total,latency,724*5)
 gc.collect();tracemalloc.start()
 for i in range(40):
  rows=v2.parse_ps(raw);rows=None
  assert v2.disk_scan(root)==(total,latency,724*5)
  if i%16==0:gc.collect()
 current,peak=tracemalloc.get_traced_memory();tracer=tracemalloc.get_tracemalloc_memory();tracemalloc.stop()
 assert peak+tracer<2*1024**2
 link=root/'bad-link';link.symlink_to(root/'worker-0')
 try:v2.disk_scan(root);raise AssertionError('symlink accepted')
 except RuntimeError as e:assert 'symlink' in str(e)
 link.unlink()
 for i in range(4096-724*5):(root/f'f-{i}').touch()
 assert v2.disk_scan(root)[2]==4096
 (root/'overflow').touch()
 try:v2.disk_scan(root);raise AssertionError('4097 files accepted')
 except RuntimeError as e:assert 'file cap' in str(e)
 (root/'overflow').unlink()
 p=root
 for i in range(64):p=p/'deep';p.mkdir()
 try:v2.disk_scan(root);raise AssertionError('unbounded depth accepted')
 except RuntimeError as e:assert 'depth cap' in str(e)
report={'scope':'synthetic observer helpers only; not gameplay or formal run','same_captured_ps_rows':len(old),'same_ps_parse':True,'synthetic_worker_directories':724,'same_disk_bytes':total,'same_latency_bytes':latency,'files':724*5,'repeated_samples':40,'current_bytes':current,'peak_bytes':peak,'tracer_bytes':tracer,'combined_observer_hot_bytes':peak+tracer,'bound_2MiB':'passed_helper_only','symlink_rejected':True,'4096_files_accepted_4097_rejected':True,'depth64_rejected':True}
(D/'observer-helper-check.json').write_text(json.dumps(report,indent=2)+'\n');print(json.dumps(report,indent=2))
