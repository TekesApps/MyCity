"""Sequential full cold input loads, without generating another pressure copy."""
import hashlib
import json
import subprocess
import time
from pathlib import Path

D = Path(__file__).resolve().parent
R = D.parents[2]
source = json.loads((D / 'source.json').read_text())
binary = Path(source['binary'])
assert hashlib.sha256(binary.read_bytes()).hexdigest() == source['binary_sha256']
cases = {
    'current7': 'work/typed-workspace-formal-l1-01/observed/model/layout/snapshots/snapshot-000007',
    'current8': 'work/typed-workspace-formal-l1-01/observed/model/layout/snapshots/snapshot-000008',
}
results = []
for name, path in cases.items():
    snapshot = R / path
    original = (snapshot / 'index.json').read_bytes()
    out = D / ('cold-' + name)
    assert not out.exists()
    command = [str(binary), 'layout-load-only', str(snapshot), str(out)]
    started = time.monotonic()
    with (D / f'cold-{name}.stdout').open('x') as stdout, (D / f'cold-{name}.stderr').open('x') as stderr:
        code = subprocess.run(command, cwd=R, stdout=stdout, stderr=stderr).returncode
    row = dict(case=name, input=path, command=command, exit_code=code,
               seconds=time.monotonic() - started, source_sha256=source['source_sha256'],
               index_sha256=hashlib.sha256(original).hexdigest(),
               original_index_unchanged=original == (snapshot / 'index.json').read_bytes(),
               new_pressure_copy=False, memory_16gb_gate='not_run', full_M0='not_accepted')
    results.append(row)
    (D / 'cold-current-results.json').write_text(json.dumps(results, indent=2) + '\n')
    print(json.dumps(row), flush=True)
    if code != 0:
        raise SystemExit(code)
