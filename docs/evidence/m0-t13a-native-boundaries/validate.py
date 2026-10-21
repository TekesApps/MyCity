"""Freeze exact Rust+fixtures, run full release regression, retain fixed executable.
Uses existing target. No formal performance claim or automatic retry on failure.
"""
import hashlib,json,os,struct,subprocess,time,zipfile
from pathlib import Path
D=Path(__file__).resolve().parent;R=D.parents[2];os.chdir(R)
def save(n,v):
 p=D/n
 with p.open('x') as f:json.dump(v,f,indent=2);f.write('\n');f.flush();os.fsync(f.fileno())
def sha(p):return hashlib.sha256(Path(p).read_bytes()).hexdigest()
files=sorted(['.cargo/config.toml','Cargo.toml','Cargo.lock','rust-toolchain.toml']+[p.relative_to(R).as_posix() for root in ['crates','fixtures'] for p in (R/root).rglob('*') if p.is_file()])
def digest():
 h=hashlib.sha256()
 for n in files:
  b=(R/n).read_bytes();name=n.encode();h.update(struct.pack('<Q',len(name)));h.update(name);h.update(struct.pack('<Q',len(b)));h.update(b)
 return h.hexdigest()
source=digest()
with zipfile.ZipFile(D/'frozen-source.zip','x',zipfile.ZIP_DEFLATED) as z:
 for n in files:z.write(R/n,n)
save('freeze.json',dict(source_sha256=source,source_files=len(files),lock_sha256=sha('Cargo.lock'),acceptance_sha256=sha('fixtures/m0-t13a-acceptance.json'),full_opening_sha256=sha('fixtures/m0-t13a-full-opening-candidate.json'),source_zip_sha256=sha(D/'frozen-source.zip'),started_unix_ns=time.time_ns(),shared_target='target/release',formal_thresholds_changed=False))
for name,cmd in [
 ('frozen-workspace',['cargo','test','--release','--workspace','--locked','--offline']),
 ('frozen-clippy',['cargo','clippy','--release','--workspace','--all-targets','--locked','--offline','--','-D','warnings']),
 ('frozen-fmt',['cargo','fmt','--all','--','--check']),
 ('frozen-build',['cargo','build','--release','--workspace','--locked','--offline','--message-format=json'])]:
 assert digest()==source
 started=time.monotonic()
 with (D/(name+'.stdout')).open('x') as o,(D/(name+'.stderr')).open('x') as e:
  p=subprocess.Popen(cmd,cwd=R,stdout=o,stderr=e)
  running=dict(controller_pid=os.getpid(),pid=p.pid,name=name,command=cmd,source_sha256=source)
  (D/'running.json').write_text(json.dumps(running,indent=2)+'\n');code=p.wait()
 assert digest()==source
 fact=dict(command=cmd,exit_code=code,seconds=time.monotonic()-started,source_sha256=source);save(name+'.json',fact);print(name,code,fact['seconds'],flush=True)
 if code:raise SystemExit(code)
fixed=R/'work/candidate-binaries/steady-native-boundaries/mycity-headless';fixed.parent.mkdir(parents=True,exist_ok=True)
assert not fixed.exists();exe=R/'target/release/mycity-headless';assert source.encode() in exe.read_bytes();os.link(exe,fixed)
save('source.json',dict(source_sha256=source,source_files=len(files),binary=str(fixed),binary_sha256=sha(fixed),lock_sha256=sha('Cargo.lock'),scope='frozen source passed full regression; actual controller diagnostic and formal gates separately reported'))
save('regression-terminal.json',dict(source_sha256=source,all_commands_exit_zero=True,elapsed_is_regression_not_performance=True))
