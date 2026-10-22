"""Actual publisher lock timeout; root exits while journal remains locked."""
import fcntl,json,subprocess,time
from pathlib import Path
D=Path(__file__).resolve().parent;R=D.parents[2];exe=json.loads((D/'source.json').read_text())['binary'];root=R/'work/native-watch-preflight-02/publisher-lock';assert not root.exists()
with (D/'publisher-lock.stdout').open('x') as o,(D/'publisher-lock.stderr').open('x') as e:
 p=subprocess.Popen([exe,'native-observe',str(root),'--',exe,'native-watch-fixture',str(root/'model'),'ok'],stdout=o,stderr=e)
 deadline=time.monotonic()+10
 while not (root/'host-samples.jsonl').exists() or not (root/'host-samples.jsonl').stat().st_size:
  assert p.poll() is None and time.monotonic()<deadline;time.sleep(.001)
 with (root/'registrations.jsonl').open('rb') as f:
  fcntl.flock(f,fcntl.LOCK_EX);began=time.monotonic()
  # Root will finish after700ms and fail its100ms publication lock deadline.
  time.sleep(1.4)
  model_stderr=(root/'model.stderr').read_text();assert 'native_registration_lock_timeout' in model_stderr
  rows=[json.loads(x) for x in (root/'host-samples.jsonl').read_text().splitlines()]
  busy=[x for x in rows if x['registration_lock_busy']];assert len(busy)>=3 and all(not x['group_capture_complete'] for x in busy)
  locked_seconds=time.monotonic()-began;fcntl.flock(f,fcntl.LOCK_UN)
 code=p.wait(timeout=15);report=json.loads((root/'observer-report.json').read_text());assert code==2 and report['model_exit_code']==2 and report['cleanup_error'] is None
(D/'publisher-lock-result.json').write_text(json.dumps(dict(exit=code,held_lock_seconds=locked_seconds,publisher_timed_out_before_release=True,busy_frames=len(busy),stderr=model_stderr,report=report,scope='functional lock deadline and failure propagation; not performance acceptance'),indent=2)+'\n');print('publisher timeout and failure propagation confirmed',flush=True)
