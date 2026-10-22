import hashlib,json,os,shutil,subprocess,time
from pathlib import Path

D=Path(__file__).resolve().parent
R=D.parents[3]
W=R/'work/core-transport-tax-main-119-attempt02'
W.mkdir(exist_ok=False)
env=os.environ.copy()
env['CARGO_TARGET_DIR']=str(R/'target')
env.pop('RUST_MIN_STACK',None)
rows=[]
commands=[
 ('build-market',['cargo','+1.85.1','build','--locked','--offline','--release','-p','mycity-core','--example','town_market']),
 ('build-transport',['cargo','+1.85.1','build','--locked','--offline','--release','-p','mycity-storage','--example','town_transport']),
 ('market-scenarios',['python3',str(R/'crates/city-core/tests/town_market_scenarios.py'),str(R/'target/release/examples/town_market')]),
 ('core-lib',['cargo','+1.85.1','test','--locked','--offline','--release','-p','mycity-core','--lib']),
 ('construction-recovery',['cargo','+1.85.1','test','--locked','--offline','--release','-p','mycity-storage','--test','construction']),
]
commands += [('transport-'+s,[str(R/'target/release/examples/town_transport'),s,str(W/s)]) for s in ('normal','closed','bottleneck','expanded')]
commands += [
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
bins=[]
for name in ('town_market','town_transport'):
 p=D/(name+'-release');shutil.copy2(R/'target/release/examples'/name,p)
 bins.append({'file':p.name,'sha256':hashlib.sha256(p.read_bytes()).hexdigest()})
(D/'binaries.json').write_text(json.dumps(bins,indent=2)+'\n')
print('Integrated main functional checks passed; process heap/performance/16GB not measured')
