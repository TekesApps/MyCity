"""Wait for this live regression, then exec original formal run once. No restart.

This process becomes the admission/observer process; it does not remain a parked
orchestration parent during measurement. Missing/failed regression is an error.
"""
import json
import os
from pathlib import Path
import subprocess
import sys
import time

D = Path(__file__).resolve().parent
running = json.loads((D/'running.json').read_text())
controller = running['controller_pid']
source = running['source_sha256']
check = subprocess.run(['ps','-p',str(controller),'-o','pid=,lstart=,comm='], capture_output=True, text=True)
if check.returncode or not check.stdout.strip():
    raise SystemExit('regression controller not live at dispatch')
(D/'after-regression-live.json').write_text(json.dumps(dict(pid=os.getpid(),
    waited_controller_pid=controller, initial_process_identity=check.stdout.strip(),
    source_sha256=source, phase='waiting for actual regression terminal; no automatic retry'),indent=2)+'\n')
while True:
    p = subprocess.run(['ps','-p',str(controller),'-o','pid=,lstart=,comm='], capture_output=True, text=True)
    if p.returncode:
        break
    if p.stdout.strip() != check.stdout.strip():
        raise SystemExit('regression controller identity changed')
    time.sleep(1)
terminal = json.loads((D/'regression-terminal.json').read_text())
if not terminal['all_commands_exit_zero'] or terminal['source_sha256'] != source:
    raise SystemExit('regression not passed at this source')
(D/'after-regression-ready.json').write_text(json.dumps(dict(pid=os.getpid(),
    source_sha256=source, actual_regression_passed=True,
    next='exec original formal admission/observer, no parked parent'),indent=2)+'\n')
os.execv(sys.executable,[sys.executable,str(D/'run-formal.py')])
