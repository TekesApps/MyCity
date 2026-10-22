import hashlib,json,subprocess,time
from pathlib import Path
D=Path(__file__).resolve().parent;R=D.parents[2];i=json.loads((D/'source.json').read_text());exe=i['binary'];assert hashlib.sha256(Path(exe).read_bytes()).hexdigest()==i['binary_sha256'];W=R/'work/r07-report-error-probe-01';assert not W.exists();t=time.monotonic();cmd=[exe,'layout-r07-session',str(W),'--report-fault'];p=subprocess.run(cmd,capture_output=True,text=True,timeout=20)
(D/'error-probe.stdout').write_text(p.stdout);(D/'error-probe.stderr').write_text(p.stderr)
assert p.returncode!=0 and 'metrics_unknown_class' in p.stderr and not (W/'report.json').exists() and not (W/'snapshots').exists()
(D/'error-probe.json').write_text(json.dumps(dict(source_sha256=i['source_sha256'],binary_sha256=i['binary_sha256'],command=cmd,exit=p.returncode,seconds=time.monotonic()-t,report_exists=False,full_layout_allocated=False,error='metrics_unknown_class',formal_acceptance=False),indent=2)+'\n');print('actual metrics error rejected',p.returncode)
