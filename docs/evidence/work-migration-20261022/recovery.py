"""Actual post-migration recovery; reads existing complete originals, writes only small new receipts."""
import json,subprocess,time,hashlib
from pathlib import Path
R=Path('/Users/frankqin/dev/MyCity');D=Path(__file__).resolve().parent;P=D/'attempt03'
assert json.loads((P/'switch.json').read_text())['all_file_bytes_verified']
assert (R/'work').is_symlink() and (R/'work').resolve()==Path('/Volumes/HP P900/dev/MyCity/work')
info=json.loads((R/'docs/evidence/m0-t13a-extents-joint/source.json').read_text());exe=Path(info['binary']);assert hashlib.sha256(exe.read_bytes()).hexdigest()==info['binary_sha256']
root=R/'work/migration-recovery-20261022';root.mkdir(exist_ok=False)
(root/'intent.json').write_text(json.dumps(dict(purpose='post-migration full original recovery checks; no pressure generation',estimated_max_new_output_bytes=1048576,storage_profile=str(D/'storage-profile.json'),input_originals_retained=True,release='keep small reports/logs as migration evidence'),indent=2)+'\n')
cases=[('typed8','work/typed-workspace-formal-l1-01/observed/model/layout/snapshots/snapshot-000008'),('legacy-v50','work/t13a50-joint-01/model/layout/snapshots/snapshot-000001'),('v3','work/r07-report-error-l1-01/observed/model/layout/snapshots/snapshot-000002')];results=[]
for name,input_path in cases:
 cmd=[str(exe),'layout-load-only',str(R/input_path),str(root/name)];start=time.monotonic()
 with (D/(name+'.recovery.stdout')).open('x') as out,(D/(name+'.recovery.stderr')).open('x') as err:q=subprocess.run(cmd,stdout=out,stderr=err)
 row=dict(case=name,command=cmd,exit=q.returncode,elapsed_observation_seconds=time.monotonic()-start,input=str(R/input_path),report=str(root/name/'report.json'),binary_sha256=info['binary_sha256'],source_sha256=info['source_sha256'],storage_profile='external_USB_APFS_HP_P900_SSD',is_performance_acceptance=False);results.append(row)
 (D/'recovery-results.json').write_text(json.dumps(results,indent=2)+'\n')
 if q.returncode!=0:raise RuntimeError(row)
 report=root/name/'report.json';assert report.is_file();(D/(name+'.recovery.report.json')).write_bytes(report.read_bytes())
 print(name,'actual new-process full cold read and scan passed',flush=True)
