from pathlib import Path
import os,sys,subprocess,time,json,hashlib,zipfile,datetime
root=Path(__file__).resolve().parent
label=sys.argv[1]; cmd=sys.argv[2:]; repo=Path.cwd(); d=root/label; d.mkdir(exist_ok=False)
scopes=['Cargo.toml','Cargo.lock','rust-toolchain.toml','.cargo','crates','fixtures','docs/construction-retry-policy.md','docs/town-lifelines-joint.md','docs/unfinished-clearance.md','docs/town-boot-electricity.md','docs/core-road-clearance.md']
def source():
 files=[]
 for name in scopes:
  p=repo/name
  for f in sorted(p.rglob('*')) if p.is_dir() else [p]:
   if f.is_symlink():raise RuntimeError('source symlink:'+str(f))
   if f.is_file():files.append(f)
 return [{'path':f.relative_to(repo).as_posix(),'size':f.stat().st_size,'sha256':hashlib.sha256(f.read_bytes()).hexdigest()} for f in sorted(set(files))]
entries=source()
(d/'source-manifest.json').write_text(json.dumps(entries,sort_keys=True,indent=2))
with zipfile.ZipFile(d/'source.zip','x',compression=zipfile.ZIP_STORED) as z:
 for row in entries:z.write(repo/row['path'],row['path'])
e=os.environ.copy();e.pop('RUST_MIN_STACK',None)
for k in tuple(e):
 if k.startswith('MYCITY_'):e.pop(k,None)
e['CARGO_TARGET_DIR']='/Volumes/HP P900/dev/MyCity/target';e['CARGO_BUILD_JOBS']='2';e['MYCITY_TEST_ARTIFACT_ROOT']=str(d/'test-artifacts')
(d/'command.json').write_text(json.dumps({'argv':cmd,'cwd':str(repo),'target':e['CARGO_TARGET_DIR'],'rust_min_stack':'unset','started_utc':datetime.datetime.now(datetime.timezone.utc).isoformat(),'source_manifest_sha256':hashlib.sha256((d/'source-manifest.json').read_bytes()).hexdigest()},indent=2))
t=time.monotonic()
with (d/'stdout.log').open('wb') as out,(d/'stderr.log').open('wb') as err:
 p=subprocess.Popen(cmd,stdout=out,stderr=err,env=e);print('child_pid='+str(p.pid),flush=True);rc=p.wait()
unchanged=source()==entries
(d/'exit.json').write_text(json.dumps({'exit_code':rc,'wall_seconds':time.monotonic()-t,'source_unchanged':unchanged},indent=2))
print('exit_code='+str(rc)+' source_unchanged='+str(unchanged),flush=True)
for name in ['stdout.log','stderr.log']:
 text=(d/name).read_text(errors='replace');print(name+':'+text[-16000:])
