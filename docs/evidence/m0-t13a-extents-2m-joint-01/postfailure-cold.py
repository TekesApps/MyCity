import json,subprocess,time,os
from pathlib import Path
D=Path(__file__).resolve().parent;R=D.parents[2];W=R/'work/m0-t13a-extents-2m-joint-l1-01';info=json.loads((D/'source.json').read_text());t=json.loads((D/'terminal.json').read_text());assert t['observer_exit']==0 and t['controller_error'];results=[]
for g in [1,2]:
 source=W/'observed/model/layout/snapshots'/f'snapshot-{g:06}';target=W/f'independent-cold-after-failure-gen{g}';cmd=[info['binary'],'layout-load-only',str(source),str(target)];start=time.monotonic()
 with (D/f'postfailure-cold-gen{g}.stdout').open('x') as out,(D/f'postfailure-cold-gen{g}.stderr').open('x') as err:q=subprocess.run(cmd,stdout=out,stderr=err,env={k:v for k,v in os.environ.items() if not k.startswith('MYCITY_')})
 report=json.loads((target/'report.json').read_text());assert q.returncode==0 and report['cold_reopen_and_scan']=='passed';results.append(dict(generation=g,exit=q.returncode,source=str(source),target=str(target),source_sha256=info['source_sha256'],elapsed_seconds=time.monotonic()-start));print('cold',g,'passed',flush=True)
(D/'cold-result.json').write_text(json.dumps(dict(all_generations_cold_restored=True,generations=results),indent=2)+'\n')
