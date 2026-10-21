"""Freeze identity through focused checks, then exec one original-load experiment.
Full same-source workspace regression is still required before acceptance.
"""
import hashlib
import json
import os
from pathlib import Path
import subprocess
import sys
import time
import zipfile

D = Path(__file__).resolve().parent
R = D.parents[2]
os.chdir(R)
info = json.loads((D/'source.json').read_text())

def check():
    with zipfile.ZipFile(D/'frozen-source.zip') as z:
        for name in z.namelist():
            assert (R/name).read_bytes() == z.read(name), name
    assert hashlib.sha256(Path(info['binary']).read_bytes()).hexdigest() == info['binary_sha256']

check()
commands = [
    ('focused-unit', ['cargo','test','--release','--locked','--offline','-p','mycity-headless','--lib','layout_load::']),
    ('focused-clippy', ['cargo','clippy','--release','--workspace','--all-targets','--locked','--offline','--','-D','warnings']),
    ('focused-fmt', ['cargo','fmt','--all','--','--check']),
]
for name, cmd in commands:
    check()
    start = time.monotonic()
    with (D/(name+'.stdout')).open('x') as out, (D/(name+'.stderr')).open('x') as err:
        process = subprocess.Popen(cmd, stdout=out, stderr=err)
        (D/'running.json').write_text(json.dumps(dict(controller_pid=os.getpid(), pid=process.pid,
            command=cmd, source_sha256=info['source_sha256']),indent=2)+'\n')
        code = process.wait()
    check()
    result = dict(command=cmd, exit_code=code, seconds=time.monotonic()-start,
                  source_sha256=info['source_sha256'])
    (D/(name+'.json')).write_text(json.dumps(result,indent=2)+'\n')
    print(name, code, result['seconds'], flush=True)
    if code:
        raise SystemExit(code)
(D/'focused-terminal.json').write_text(json.dumps(dict(source_sha256=info['source_sha256'],
    all_commands_exit_zero=True, full_workspace='not_run_yet_required_before_acceptance',
    fixed_binary_unchanged=True),indent=2)+'\n')
os.execv(sys.executable, [sys.executable, str(D/'run-formal.py')])
