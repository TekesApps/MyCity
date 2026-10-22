from pathlib import Path
import json,hashlib,subprocess,time
r=Path(__file__).resolve().parent;d=r/'68-legacy-reader';d.mkdir(exist_ok=False)
def sha(p):return hashlib.sha256(p.read_bytes()).hexdigest()
def manifest(p):
 rows=[]
 for f in sorted(p.rglob('*')):
  assert not f.is_symlink()
  if f.is_file():rows.append({'path':str(f.relative_to(p)),'size':f.stat().st_size,'sha256':sha(f)})
 assert rows
 return rows
repo=Path('/Users/frankqin/dev/MyCity');prep=repo/'docs/evidence/core-road-legacy-fixture-preparation';old=Path('/Volumes/HP P900/dev/MyCity/work/legacy-accepted-project-119-run01');report=json.loads((old/'report.json').read_text())
pin=json.loads((r/'49-cli-rebuild/cli-executables.json').read_text())['town_transport'];exe=Path(pin['preserved']);assert sha(exe)==pin['sha256']
cases=[]
for kind,key in [('accepted-wal','wal'),('accepted-checkpoint','checkpoint')]:
 seal=json.loads((prep/'attempt01'/f'{kind}.seal.json').read_text());before=manifest(old/kind)
 assert before==json.loads((prep/'attempt01'/f'{kind}.manifest.json').read_text())['files']
 assert seal['state_sha256']==report[key]['state_sha256']
 cases.append((kind,old/kind,'construction-engine-v1',report[key]['state_sha256']))
t=repo/'work/core-transport-tax-main-119-attempt02/normal';e=repo/'docs/evidence/core-transport-tax-integration-119/attempt02/transport-normal.stdout';h=json.loads(e.read_text())['state_sha256']
cases.append(('transport-v119',t,'m0-town-transport-fixed-profile-v1',h))
results=[]
for label,source,rules,h in cases:
 o=d/label;o.mkdir();before=manifest(source);(o/'original-before.json').write_text(json.dumps(before,indent=2))
 occupied=subprocess.run(['lsof','-nP','+D',str(source)],capture_output=True,text=True)
 assert occupied.returncode==1 and not occupied.stdout and not occupied.stderr,occupied
 dest=o/'world';subprocess.run(['/bin/cp','-cRp',str(source),str(dest)],check=True);assert manifest(dest)==before
 argv=['/usr/bin/time','-l',str(exe),'reopen-check',str(dest),rules,h];start=time.monotonic()
 with (o/'stdout.log').open('xb') as out,(o/'stderr.log').open('xb') as err:p=subprocess.run(argv,stdout=out,stderr=err)
 after=manifest(source);(o/'original-after.json').write_text(json.dumps(after,indent=2));(o/'copy-after.json').write_text(json.dumps(manifest(dest),indent=2))
 row={'case':label,'argv':argv,'exit_code':p.returncode,'wall_seconds':time.monotonic()-start,'source_original_unchanged':before==after,'copy_before_equal':True,'expected_state_sha256':h,'executable':pin,'build_source_manifest_sha256':sha(r/'49-cli-rebuild/source-manifest.json'),'storage':'HP P900 external USB/APFS SSD','scope':'functional compatibility cold read on full copies only; not M0 performance'}
 (o/'receipt.json').write_text(json.dumps(row,indent=2));results.append(row);print(label,p.returncode,(o/'stdout.log').read_text(),flush=True)
 assert p.returncode==0 and before==after
(d/'summary.json').write_text(json.dumps(results,indent=2))
