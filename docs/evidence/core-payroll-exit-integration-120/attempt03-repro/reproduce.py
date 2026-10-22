import hashlib,json,os,shutil,subprocess,time,zipfile
from pathlib import Path
D=Path(__file__).resolve().parent;R=D.parents[3]
p=R/'crates/city-storage/src/checkpoint/receipt_decode.rs';fixed=p.read_bytes()
env=os.environ.copy();env['CARGO_TARGET_DIR']=str(R/'target');env.pop('RUST_MIN_STACK',None)
a=R/'work/payroll-exit-main-120-repro03';a.mkdir();env['MYCITY_TEST_ARTIFACT_ROOT']=str(a.resolve())
try:
 p.write_bytes(subprocess.check_output(['git','show','HEAD:crates/city-storage/src/checkpoint/receipt_decode.rs'],cwd=R))
 rows=[]
 with zipfile.ZipFile(D/'source.zip','x',compression=zipfile.ZIP_DEFLATED) as z:
  for x in json.loads((D.parent/'attempt02/freeze.json').read_text())['files']:
   b=(R/x['path']).read_bytes();rows.append({'path':x['path'],'sha256':hashlib.sha256(b).hexdigest()});z.writestr(x['path'],b)
 digest=hashlib.sha256(json.dumps(rows,sort_keys=True,separators=(',',':')).encode()).hexdigest()
 (D/'freeze.json').write_text(json.dumps({'source_sha256':digest,'algorithm':'sha256 canonical compact sorted-key JSON files array','files':rows,'note':'intentional pre-fix decoder with retained failure fixtures'},indent=2)+'\n')
 cmd=['cargo','+1.85.1','test','--locked','--offline','--release','-p','mycity-storage','--test','payroll_exit','payroll_exit_checkpoint_reopen_replay_and_explicit_settlement_conserve_cash','--','--exact','--nocapture']
 t=time.monotonic()
 with (D/'test.stdout').open('x') as out,(D/'test.stderr').open('x') as err:q=subprocess.run(cmd,cwd=R,env=env,stdout=out,stderr=err)
 exe=R/'target/release/deps/payroll_exit-f90fbeceff047160';dst=D/'failed-payroll-exit-test';shutil.copy2(exe,dst)
 result={'command':cmd,'exit':q.returncode,'seconds':time.monotonic()-t,'artifacts':str(a.resolve()),'binary_sha256':hashlib.sha256(dst.read_bytes()).hexdigest(),'original_attempt02_fixture':'not retained by old unconditional test Drop; this is a new faithful reproduction'}
 (D/'result.json').write_text(json.dumps(result,indent=2)+'\n');print(json.dumps(result));assert q.returncode!=0
 assert 'invalid_json' in (D/'test.stdout').read_text()+(D/'test.stderr').read_text()
 assert list(a.glob('mycity-payroll-exit-*/world'))
finally:p.write_bytes(fixed)
