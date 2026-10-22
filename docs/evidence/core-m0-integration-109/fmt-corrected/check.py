"""Single serialized functional check; no formal performance workload."""
import hashlib,json,os,struct,subprocess,time,zipfile
from pathlib import Path
D=Path(__file__).resolve().parent; R=D.parents[3]; os.chdir(R)
files=sorted(['.cargo/config.toml','Cargo.toml','Cargo.lock','rust-toolchain.toml']+[p.relative_to(R).as_posix() for base in ['crates','fixtures'] for p in (R/base).rglob('*') if p.is_file()])
def digest():
 h=hashlib.sha256()
 for n in files:
  b=(R/n).read_bytes();name=n.encode();h.update(struct.pack('<Q',len(name)));h.update(name);h.update(struct.pack('<Q',len(b)));h.update(b)
 return h.hexdigest()
def save(n,v):(D/n).write_text(json.dumps(v,indent=2)+'\n')
source=digest(); assert not (D/'freeze.json').exists()
with zipfile.ZipFile(D/'source.zip','x',zipfile.ZIP_DEFLATED) as z:
 for n in files:z.write(R/n,n)
save('freeze.json',{'source_sha256':source,'source_files':len(files),'source_zip_sha256':hashlib.sha256((D/'source.zip').read_bytes()).hexdigest(),'shared_target':str(R/'target'),'formal_performance':False,'full_M0':'not_accepted','real16GB':'not_run','eight_hour':'not_run'})
commands=[
 ('libraries',['cargo','test','--release','--locked','--offline','-p','mycity-core','-p','mycity-headless','--lib']),
 ('market-access',['cargo','test','--release','--locked','--offline','-p','mycity-core','--test','market_access']),
 ('construction-core',['cargo','test','--release','--locked','--offline','-p','mycity-core','--test','construction','--test','construction_engine']),
 ('storage',['cargo','test','--release','--locked','--offline','-p','mycity-storage','--test','construction','--test','town_market']),
 ('strict',['cargo','clippy','--release','--locked','--offline','-p','mycity-core','-p','mycity-storage','-p','mycity-headless','--all-targets','--','-D','warnings']),
 ('build',['cargo','build','--release','--locked','--offline','-p','mycity-headless']),
 ('examples',['cargo','build','--release','--locked','--offline','-p','mycity-core','--examples']),
 ('fmt',['cargo','fmt','--all','--','--check'])]
results=[]
for name,cmd in commands:
 start=time.monotonic()
 with (D/(name+'.stdout')).open('x') as o,(D/(name+'.stderr')).open('x') as e:rc=subprocess.run(cmd,stdout=o,stderr=e).returncode
 assert digest()==source,'source drift'
 r={'command':cmd,'exit':rc,'seconds':time.monotonic()-start,'source_sha256':source};results.append(r);save(name+'.json',r);print(name,rc,flush=True)
 if rc:raise SystemExit(rc)
save('terminal.json',{'commands':results,'source_sha256':source,'functional_checks_passed':True,'formal_performance':'not_run','full_M0':'not_accepted'})
