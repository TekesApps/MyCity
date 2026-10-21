"""Audit only after the actual observer and model have exited; preserve raw data."""
import hashlib
import json
import subprocess
import sys
import time
from pathlib import Path

D = Path(__file__).resolve().parent
R = D.parents[2]
W = R / 'work/typed-workspace-formal-l1-01/observed'
observer = json.loads((W/'observer-report.json').read_text())
assert observer['model_exit_code'] == 0 and observer['binary_unchanged']
info = json.loads((D/'source.json').read_text())
assert observer['binary_sha256'] == info['binary_sha256']
assert json.loads((W/'model/report.json').read_text())['source_sha256'] == info['source_sha256']
results = []
scenario = json.loads((W/'model/business/report.json').read_text())['scenario']
for name, argument in [('window_audit.py',W/'model'), ('resource_window_audit.py',W),
                       ('quiescent_audit.py',W), ('layout_residency_audit.py',W)]:
    script = D/name
    if name == 'quiescent_audit.py' and scenario != 'l2':
        results.append(dict(script=name,status='not_applicable',reason='L2 daily business boundaries require an actual measurement transcript; L1 is storage pressure'))
        continue
    prefix = name.removesuffix('.py')
    start = time.monotonic()
    with (D/(prefix+'.json')).open('x') as out, (D/(prefix+'.stderr')).open('x') as err:
        code = subprocess.run([sys.executable,str(script),str(argument)],cwd=R,stdout=out,stderr=err).returncode
    result = dict(script=name,sha256=hashlib.sha256(script.read_bytes()).hexdigest(),
                  exit_code=code,seconds=time.monotonic()-start)
    results.append(result)
    (D/'postrun-results.json').write_text(json.dumps(results,indent=2)+'\n')
    print(json.dumps(result),flush=True)
    if code:
        raise SystemExit(code)
