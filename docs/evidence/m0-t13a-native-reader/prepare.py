"""Freeze/build a native reader before full cold compatibility experiments."""
import hashlib,json,os,struct,subprocess,time,zipfile
from pathlib import Path
D=Path(__file__).resolve().parent;R=D.parents[2];os.chdir(R)
files=sorted(['.cargo/config.toml','Cargo.toml','Cargo.lock','rust-toolchain.toml']+[p.relative_to(R).as_posix() for root in ['crates','fixtures'] for p in (R/root).rglob('*') if p.is_file()])
def digest():
 h=hashlib.sha256()
 for n in files:
  b=(R/n).read_bytes();s=n.encode();h.update(struct.pack('<Q',len(s)));h.update(s);h.update(struct.pack('<Q',len(b)));h.update(b)
 return h.hexdigest()
sha=lambda p:hashlib.sha256(Path(p).read_bytes()).hexdigest()
source=digest();assert not (D/'freeze.json').exists()
with zipfile.ZipFile(D/'frozen-source.zip','x',zipfile.ZIP_DEFLATED) as z:
 for n in files:z.write(R/n,n)
(D/'freeze.json').write_text(json.dumps({'source_sha256':source,'source_files':len(files),'lock_sha256':sha('Cargo.lock'),'acceptance_sha256':sha('fixtures/m0-t13a-acceptance.json'),'full_opening_sha256':sha('fixtures/m0-t13a-full-opening-candidate.json'),'source_zip_sha256':sha(D/'frozen-source.zip'),'started_unix_ns':time.time_ns(),'shared_target':'target/release','original_load_thresholds_changed':False},indent=2)+'\n')
cmd=['cargo','build','--release','--locked','--offline','-p','mycity-headless'];started=time.monotonic()
with (D/'build.stdout').open('x') as o,(D/'build.stderr').open('x') as e:
 code=subprocess.run(cmd,stdout=o,stderr=e).returncode
assert digest()==source
(D/'build.json').write_text(json.dumps({'command':cmd,'exit_code':code,'seconds':time.monotonic()-started,'source_sha256':source},indent=2)+'\n')
if code:raise SystemExit(code)
fixed=R/'work/candidate-binaries/native-reader/mycity-headless';fixed.parent.mkdir(parents=True,exist_ok=True);assert not fixed.exists();assert source.encode() in (R/'target/release/mycity-headless').read_bytes();os.link(R/'target/release/mycity-headless',fixed)
(D/'source.json').write_text(json.dumps({'source_sha256':source,'source_files':len(files),'binary':str(fixed),'binary_sha256':sha(fixed),'lock_sha256':sha('Cargo.lock'),'scope':'Frozen native-reader backend probe; formal observer integration and full same-source regression incomplete'},indent=2)+'\n');print(source,flush=True)
