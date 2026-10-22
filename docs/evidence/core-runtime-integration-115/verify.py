import json,os,subprocess,time,hashlib,shutil
from pathlib import Path
D=Path(__file__).resolve().parent
R=D.parents[2]
env=os.environ.copy();env['CARGO_TARGET_DIR']=str(R/'target');env.pop('RUST_MIN_STACK',None)
commands=[('core-tests',['cargo','+1.85.1','test','-p','mycity-core','--release','--locked','--offline','--test','construction','--test','construction_engine','--test','market_access','--test','town_business_exit']),('store-tests',['cargo','+1.85.1','test','-p','mycity-storage','--release','--locked','--offline','--test','construction']),('construction-cli',['cargo','+1.85.1','run','-p','mycity-core','--release','--locked','--offline','--example','construction']),('strict',['cargo','+1.85.1','clippy','-p','mycity-core','-p','mycity-storage','--all-targets','--release','--locked','--offline','--','-D','warnings']),('fmt',['cargo','+1.85.1','fmt','--all','--check'])]
rows=[]
for name,cmd in commands:
 start=time.monotonic()
 with (D/(name+'.log')).open('x') as f:p=subprocess.run(cmd,cwd=R,env=env,stdout=f,stderr=subprocess.STDOUT)
 rows.append(dict(name=name,command=cmd,exit=p.returncode,elapsed_seconds=time.monotonic()-start,RUST_MIN_STACK='unset'))
 (D/'results.json').write_text(json.dumps(rows,indent=2)+'\n')
 if p.returncode:raise SystemExit(p.returncode)
exe=R/'target/release/examples/construction';fixed=D/'construction-release';shutil.copy2(exe,fixed)
(D/'fixed-exe.json').write_text(json.dumps({'sha256':hashlib.sha256(fixed.read_bytes()).hexdigest(),'bytes':fixed.stat().st_size,'source':json.loads((D/'freeze.json').read_text())['source_sha256'],'path':str(fixed)},indent=2)+'\n')
print('All five limited main-branch commands passed; not whole workspace or performance')
