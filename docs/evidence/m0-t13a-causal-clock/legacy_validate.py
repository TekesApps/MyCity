"""Actual complete v50 cold restore; same frozen source, no resave/copy of inputs."""
import hashlib,json,os,struct,subprocess,time,zipfile
from pathlib import Path
D=Path(__file__).resolve().parent;R=D.parents[2];os.chdir(R)
assert json.loads((D/'build.json').read_text())['exit_code']==0
s=json.loads((D/'source.json').read_text());freeze=json.loads((D/'freeze.json').read_text())
with zipfile.ZipFile(D/'frozen-source.zip') as z:names=sorted(z.namelist())
def digest():
 h=hashlib.sha256()
 for n in names:
  b=Path(n).read_bytes();name=n.encode();h.update(struct.pack('<Q',len(name)));h.update(name);h.update(struct.pack('<Q',len(b)));h.update(b)
 return h.hexdigest()
assert digest()==s['source_sha256']
assert hashlib.sha256(Path(s['binary']).read_bytes()).hexdigest()==s['binary_sha256']
root=Path('work/causal-clock-legacy-v50-01');assert not root.exists();root.mkdir()
intent=dict(purpose='complete original v50 old-format cold restore after fixed-region activation',input='work/t13a50-joint-01/model/layout/snapshots/snapshot-000001',new_pressure_snapshot=False,retention='small report/raw/heap evidence; original v50 input remains complete for future compatibility')
(root/'retention-intent.json').write_text(json.dumps(intent,indent=2)+'\n')
cmd=[s['binary'],'layout-load-only',str(R/intent['input']),str(R/root/'restored')]
env=os.environ.copy();env['MYCITY_HEAP_LOG']=str(R/root/'heaps.jsonl')
start=time.monotonic()
with (D/'legacy-v50.stdout').open('x') as out,(D/'legacy-v50.stderr').open('x') as err:
 p=subprocess.Popen(cmd,stdout=out,stderr=err,env=env)
 (D/'legacy-running.json').write_text(json.dumps(dict(pid=p.pid,command=cmd),indent=2)+'\n');code=p.wait()
fact=dict(command=cmd,exit_code=code,seconds=time.monotonic()-start,source_sha256=s['source_sha256'],source_unchanged=digest()==s['source_sha256'],fixed_binary_unchanged=hashlib.sha256(Path(s['binary']).read_bytes()).hexdigest()==s['binary_sha256'],scope='actual full old-format cold restore, not group memory or throughput acceptance')
(D/'legacy-v50.json').write_text(json.dumps(fact,indent=2)+'\n')
assert code==0 and fact['source_unchanged'] and fact['fixed_binary_unchanged'],fact
report=json.loads((root/'restored/report.json').read_text());assert report['cold_reopen_and_scan']=='passed'
(D/'legacy-v50-report.json').write_text(json.dumps(report,indent=2)+'\n')
print(json.dumps(fact,indent=2))
