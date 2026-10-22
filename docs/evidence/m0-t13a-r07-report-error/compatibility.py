"""Full cold originals, no pressure copy; functional only, no timing acceptance."""
import hashlib,json,os,subprocess,time
from pathlib import Path
D=Path(__file__).resolve().parent;R=D.parents[2];os.chdir(R)
i=json.loads((D/'source.json').read_text());exe=i['binary'];assert hashlib.sha256(Path(exe).read_bytes()).hexdigest()==i['binary_sha256']
W=R/'work/r07-report-error-compat-01';W.mkdir();(W/'intent.json').write_text(json.dumps(dict(purpose='read complete existing v1/v2/v3 originals with new frozen binary; no copied payload',estimated_max_new_bytes=1048576,retention='retain reports/logs; existing complete originals unchanged'))+'\n')
rows=[];env={k:v for k,v in os.environ.items() if not k.startswith('MYCITY_')}
for name,path in [('v50','work/t13a50-joint-01/model/layout/snapshots/snapshot-000001'),('typed-v98','work/typed-workspace-formal-l1-01/observed/model/layout/snapshots/snapshot-000008'),('v3-predecessor','work/r07-coupling-l1-01/observed/model/layout/snapshots/snapshot-000001')]:
 source=R/path;idx=hashlib.sha256((source/'index.json').read_bytes()).hexdigest();cmd=[exe,'layout-load-only',str(source),str(W/name)];t=time.monotonic()
 with (D/(name+'-cold.stdout')).open('x') as o,(D/(name+'-cold.stderr')).open('x') as e:
  p=subprocess.Popen(cmd,env=env,stdout=o,stderr=e);code=p.wait(timeout=120)
 fact=dict(name=name,source=path,index_sha256=idx,pid=p.pid,exit=code,seconds=time.monotonic()-t,report_path=str(W/name/'report.json'));rows.append(fact)
 (D/'compatibility-result.json').write_text(json.dumps(dict(source_sha256=i['source_sha256'],binary_sha256=i['binary_sha256'],runs=rows),indent=2)+'\n')
 assert code==0,fact
 r=json.loads((W/name/'report.json').read_text());assert r['cold_reopen_and_scan']=='passed'
 if name=='v3-predecessor':assert r['manifest']['r07']['replaced_bytes']==0 and r['manifest']['r07']['reading_sequence']==0
 else:assert r['manifest']['r07'] is None
 assert hashlib.sha256((source/'index.json').read_bytes()).hexdigest()==idx
 print(name,code,fact['seconds'],flush=True)
