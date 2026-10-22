"""Run once after owner commit integration; serial functional checks, not performance acceptance."""
import hashlib,json,struct,subprocess,time,os,zipfile,re
from pathlib import Path
R=Path('/Users/frankqin/dev/MyCity');D=Path(__file__).resolve().parent;os.chdir(R)
files=sorted(['.cargo/config.toml','Cargo.toml','Cargo.lock','rust-toolchain.toml']+[p.relative_to(R).as_posix() for b in ['crates','fixtures'] for p in (R/b).rglob('*') if p.is_file()])
def digest():
 h=hashlib.sha256()
 for n in files:
  b=(R/n).read_bytes();name=n.encode();h.update(struct.pack('<Q',len(name)));h.update(name);h.update(struct.pack('<Q',len(b)));h.update(b)
 return h.hexdigest()
def save(n,v):(D/n).write_text(json.dumps(v,indent=2)+'\n')
assert not (D/'freeze.json').exists();source=digest();tmp=R/'work/core-store28-integration-111';tmp.mkdir();(tmp/'tmp').mkdir();env=dict(os.environ,CARGO_TARGET_DIR=str(R/'target'),TMPDIR=str(tmp/'tmp'))
(tmp/'intent.json').write_text(json.dumps(dict(purpose='affected core/Store functional integration tests',estimated_new_bytes=16777216,no_pressure=True,no_new_target=True,storage='external HP P900 USB/APFS SSD',release='keep small raw receipts; retire only proven reusable helper/cache at terminal'),indent=2)+'\n')
with zipfile.ZipFile(D/'source.zip','x',zipfile.ZIP_DEFLATED) as z:
 for n in files:z.write(R/n,n)
save('freeze.json',dict(source_sha256=source,source_files=len(files),source_zip_sha256=hashlib.sha256((D/'source.zip').read_bytes()).hexdigest(),work_real=str((R/'work').resolve()),target_real=str((R/'target').resolve()),temp_real=str((tmp/'tmp').resolve()),storage='external HP P900 USB/APFS SSD',formal_performance=False,real16GB='not_run',eight_hour='not_run'))
commands=[('core',['cargo','+1.85.1','test','--release','--locked','--offline','-p','mycity-core','--lib']),('store',['cargo','+1.85.1','test','--release','--locked','--offline','-p','mycity-storage','--test','town_market']),('strict',['cargo','+1.85.1','clippy','--release','--locked','--offline','-p','mycity-core','-p','mycity-storage','--all-targets','--','-D','warnings']),('fmt',['cargo','+1.85.1','fmt','--all','--','--check'])];rows=[]
for name,cmd in commands:
 start=time.monotonic()
 with (D/(name+'.stdout')).open('x') as out,(D/(name+'.stderr')).open('x') as err:q=subprocess.run(cmd,stdout=out,stderr=err,env=env)
 row=dict(command=cmd,exit=q.returncode,elapsed_seconds=time.monotonic()-start,source_sha256=source);rows.append(row);save(name+'.json',row);assert digest()==source
 print(name,q.returncode,flush=True)
 if q.returncode:raise SystemExit(q.returncode)
E=Path('/Volumes/HP P900/dev/MyCity/acceptance-binaries')/source;E.mkdir();L=R/'work/candidate-binaries/core-store28'/source;L.parent.mkdir(parents=True,exist_ok=True);L.symlink_to(E,target_is_directory=True);exes=[]
for n in ['core','store']:
 for p in re.findall(r'Running [^\n]*\(([^)]+/release/deps/[^)]+)\)',(D/(n+'.stderr')).read_text()):
  src=R/p;dst=E/src.name;os.link(src,dst);exes.append(dict(fixed=str(L/src.name),physical=str(dst),sha256=hashlib.sha256(dst.read_bytes()).hexdigest(),same_inode_with_target=src.stat().st_ino==dst.stat().st_ino))
assert len(exes)==2
save('fixed-executables.json',exes);save('terminal.json',dict(source_sha256=source,checks=rows,functional_passed=True,formal_performance='not_run',full_M0='not_accepted'));print('Affected checks passed; fixed test executables preserved with no duplicate payload.')
