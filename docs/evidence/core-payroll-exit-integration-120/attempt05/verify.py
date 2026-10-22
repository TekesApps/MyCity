import hashlib,json,os,shutil,subprocess,time
from pathlib import Path

D=Path(__file__).resolve().parent
R=D.parents[3]
W=R/'work/payroll-exit-main-120-attempt05'
env=os.environ.copy();env['CARGO_TARGET_DIR']=str(R/'target');env.pop('RUST_MIN_STACK',None)
A=R/'work/payroll-exit-main-120-tests05';A.mkdir();env['MYCITY_TEST_ARTIFACT_ROOT']=str(A.resolve())
rows=[]
commands=[
 ('core-timeline',['cargo','+1.85.1','test','--locked','--offline','--release','-p','mycity-core','--test','timeline']),
 ('store-payroll-exit',['cargo','+1.85.1','test','--locked','--offline','--release','-p','mycity-storage','--test','payroll_exit']),
 ('storage-lib',['cargo','+1.85.1','test','--locked','--offline','--release','-p','mycity-storage','--lib']),
 ('build-cli',['cargo','+1.85.1','build','--locked','--offline','--release','-p','mycity-storage','--example','payroll_exit']),
 ('cli',[str(R/'target/release/examples/payroll_exit'),str(W)]),
 ('core-lib',['cargo','+1.85.1','test','--locked','--offline','--release','-p','mycity-core','--lib']),
 ('strict',['cargo','+1.85.1','clippy','--locked','--offline','--release','-p','mycity-core','-p','mycity-storage','--all-targets','--','-D','warnings']),
 ('fmt',['cargo','+1.85.1','fmt','--all','--check']),
]
for name,cmd in commands:
 start=time.monotonic()
 with (D/(name+'.stdout')).open('x') as out,(D/(name+'.stderr')).open('x') as err:
  p=subprocess.run(cmd,cwd=R,env=env,stdout=out,stderr=err)
 rows.append({'name':name,'command':cmd,'exit':p.returncode,'seconds':time.monotonic()-start})
 (D/'results.json').write_text(json.dumps(rows,indent=2)+'\n')
 if p.returncode:raise SystemExit(p.returncode)
for item in json.loads((D/'freeze.json').read_text())['files']:
 assert hashlib.sha256((R/item['path']).read_bytes()).hexdigest()==item['sha256'],item['path']
fixed=D/'payroll-exit-release';shutil.copy2(R/'target/release/examples/payroll_exit',fixed)
(D/'binary.json').write_text(json.dumps({'file':fixed.name,'sha256':hashlib.sha256(fixed.read_bytes()).hexdigest()},indent=2)+'\n')
print('Payroll termination primitive verified; company liquidation and legal closure remain incomplete')
