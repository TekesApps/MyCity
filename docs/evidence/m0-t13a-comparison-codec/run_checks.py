#!/usr/bin/env python3
"""Freeze source once, run exact final regression/checks serially, reuse target."""
import hashlib,json,os,struct,subprocess,time,zipfile
from pathlib import Path
D=Path(__file__).resolve().parent;R=D.parents[2]
def save(name,value):
 p=D/name;t=p.with_suffix(p.suffix+'.tmp');t.write_text(json.dumps(value,indent=2)+'\n');t.replace(p)
files=sorted(['.cargo/config.toml','Cargo.toml','Cargo.lock','rust-toolchain.toml']+[p.relative_to(R).as_posix() for root in ['crates','fixtures'] for p in (R/root).rglob('*') if p.is_file()])
def digest():
 h=hashlib.sha256()
 for n in files:
  k=n.encode();b=(R/n).read_bytes();h.update(struct.pack('<Q',len(k)));h.update(k);h.update(struct.pack('<Q',len(b)));h.update(b)
 return h.hexdigest()
source=digest();assert not (D/'source.zip').exists()
with zipfile.ZipFile(D/'source.zip','w',compression=zipfile.ZIP_DEFLATED) as z:
 for n in files:z.write(R/n,n)
save('freeze.json',dict(source_sha256=source,source_files=len(files),lock_sha256=hashlib.sha256((R/'Cargo.lock').read_bytes()).hexdigest(),fixture_sha256=hashlib.sha256((R/'fixtures/m0-t13a-acceptance.json').read_bytes()).hexdigest(),opening_fixture_sha256=hashlib.sha256((R/'fixtures/m0-t13a-full-opening-candidate.json').read_bytes()).hexdigest(),controller_pid=os.getpid(),started_unix_ns=time.time_ns(),new_target=False))
commands=[
 ('workspace-release',['cargo','test','--workspace','--release','--locked','--offline']),
 ('clippy-final',['cargo','clippy','--workspace','--all-targets','--release','--locked','--offline','--','-D','warnings']),
 ('fmt-final',['cargo','fmt','--all','--','--check']),
 ('build-final',['cargo','build','--workspace','--release','--locked','--offline'])]
checks={}
for name,cmd in commands:
 assert digest()==source,'source changed during validation'
 t=time.monotonic()
 with (D/(name+'.stdout')).open('x') as out,(D/(name+'.stderr')).open('x') as err:
  p=subprocess.Popen(cmd,cwd=R,stdout=out,stderr=err)
  save('running.json',dict(controller_pid=os.getpid(),child_pid=p.pid,name=name,command=cmd))
  code=p.wait()
 checks[name]=dict(command=cmd,exit=code,seconds=time.monotonic()-t,source_sha256=source)
 save('checks-final.json',checks);print(name,code,checks[name]['seconds'],flush=True)
 assert digest()==source,'source changed during validation'
 if code:raise SystemExit(code)
fixed=R/'work/candidate-binaries/comparison-codec/mycity-headless';fixed.parent.mkdir(parents=True,exist_ok=True)
exe=R/'target/release/mycity-headless';data=exe.read_bytes();assert source.encode() in data
os.link(exe,fixed)
info=json.loads((D/'freeze.json').read_text());info.update(binary=str(fixed.relative_to(R)),binary_sha256=hashlib.sha256(data).hexdigest(),scope='Final exact regression/build; no new joint/performance gate accepted')
save('source.json',info)
print('final source, regression and fixed executable complete',flush=True)
