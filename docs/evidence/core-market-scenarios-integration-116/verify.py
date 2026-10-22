import json,os,subprocess,time,hashlib,shutil
from pathlib import Path
D=Path(__file__).resolve().parent;R=D.parents[2]
env=os.environ.copy();env['CARGO_TARGET_DIR']=str(R/'target');env.pop('RUST_MIN_STACK',None)
rows=[]
commands=[('build',['cargo','+1.85.1','build','-p','mycity-core','--example','town_market','--release','--locked','--offline']),('scenarios',['python3',str(R/'crates/city-core/tests/town_market_scenarios.py'),str(R/'target/release/examples/town_market')]),('strict',['cargo','+1.85.1','clippy','-p','mycity-core','--example','town_market','--release','--locked','--offline','--','-D','warnings']),('fmt',['cargo','+1.85.1','fmt','--all','--check'])]
for name,cmd in commands:
 start=time.monotonic()
 with(D/(name+'.log')).open('x')as f:p=subprocess.run(cmd,cwd=R,env=env,stdout=f,stderr=subprocess.STDOUT)
 rows.append(dict(name=name,command=cmd,exit=p.returncode,seconds=time.monotonic()-start));(D/'results.json').write_text(json.dumps(rows,indent=2)+'\n')
 if p.returncode:raise SystemExit(p.returncode)
fixed=D/'town-market-release';shutil.copy2(R/'target/release/examples/town_market',fixed)
(D/'binary.json').write_text(json.dumps(dict(path=str(fixed),sha256=hashlib.sha256(fixed.read_bytes()).hexdigest()),indent=2)+'\n')
for row in json.loads((D/'freeze.json').read_text())['files']:assert hashlib.sha256((R/row['path']).read_bytes()).hexdigest()==row['sha256'],row['path']
print('7 scenario profiles plus parameter override passed on integrated main source')
