"""Actually cross the60s periodic boundary; functional diagnostics, not8h."""
import hashlib,json,subprocess,time
from pathlib import Path
D=Path(__file__).resolve().parent;R=D.parents[2];info=json.loads((D/'source.json').read_text());exe=info['binary'];W=R/'work/native-observer-periodic-01';assert not W.exists();W.mkdir();root=W/'observed'
cmd=[exe,'native-observe',str(root),'--',exe,'native-watch-fixture',str(root/'model'),'periodic'];start=time.monotonic()
with (D/'periodic.stdout').open('x') as o,(D/'periodic.stderr').open('x') as e:
 p=subprocess.Popen(cmd,stdout=o,stderr=e);(D/'periodic-running.json').write_text(json.dumps(dict(controller_pid=__import__('os').getpid(),pid=p.pid,command=cmd))+'\n');code=p.wait(timeout=90)
rows=[json.loads(x) for x in (root/'host-samples.jsonl').read_text().splitlines()];periodic=[(i,x) for i,x in enumerate(rows) if x['system'] is not None];assert code==0 and len(periodic)==2
assert periodic[0][1]['clock_begin_ns']<1_000_000_000 and periodic[1][1]['clock_begin_ns']>=60_000_000_000
assert all(x['cpu_io_footprint'] is not None for _,x in periodic)
assert all(x['system']['vm_page_size_bytes']==16384 and x['pressure_dispatch_flag'] in [1,2,4] for _,x in periodic)
assert hashlib.sha256(Path(exe).read_bytes()).hexdigest()==info['binary_sha256']
(D/'periodic-result.json').write_text(json.dumps(dict(exit=code,actual_seconds=time.monotonic()-start,source_sha256=info['source_sha256'],periodic_frame_indices=[i for i,_ in periodic],periodic_begin_ns=[x['clock_begin_ns'] for _,x in periodic],samples=len(rows),raw_cpu_counters=True,scope='actual60s crossing only; eight_hour_not_run'),indent=2)+'\n')
print((D/'periodic-result.json').read_text(),flush=True)
