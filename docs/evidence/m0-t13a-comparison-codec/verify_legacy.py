#!/usr/bin/env python3
"""Real old Store open with current exact libraries; do not mutate the original."""
import hashlib,json,shutil,subprocess,time
from pathlib import Path
R=Path(__file__).resolve().parents[3];D=Path(__file__).resolve().parent
assert all(x['exit']==0 for x in json.loads((D/'checks-final.json').read_text()).values())
assert set(json.loads((D/'checks-final.json').read_text()))=={'workspace-release','clippy-final','fmt-final','build-final'}
old=R/'docs/evidence/m0-t13a-model-workspace';before=json.loads((old/'compat-proof.json').read_text())
manifest=json.loads((old/'legacy-input-manifest.json').read_text())
original=R/before['source_original'];copy=R/before['copy_complete_retained_at']
def inspect(base):
 files={p.relative_to(base).as_posix():p for p in base.rglob('*') if p.is_file()}
 assert set(files)==set(manifest),'complete original/copied file set changed'
 for n,p in files.items():
  b=p.read_bytes();assert len(b)==manifest[n]['bytes'] and hashlib.sha256(b).hexdigest()==manifest[n]['sha256'],n
 return len(files)
assert inspect(original)==inspect(copy)==243
cmd=['cargo','build','--release','--locked','--offline','-p','mycity-storage','--message-format=json']
with (D/'compat-build-artifacts.jsonl').open('x') as out,(D/'compat-build.stderr').open('x') as err:
 assert subprocess.run(cmd,cwd=R,stdout=out,stderr=err).returncode==0
libs={}
for line in (D/'compat-build-artifacts.jsonl').read_text().splitlines():
 x=json.loads(line)
 if x.get('reason')=='compiler-artifact' and x['target']['name'] in ['mycity_core','mycity_storage']:
  libs[x['target']['name']]=next(f for f in x['filenames'] if f.endswith('.rlib'))
assert len(libs)==2
shutil.copyfile(old/'compat-restore.rs',D/'compat-restore.rs')
exe=R/'work/candidate-binaries/comparison-codec/compat-restore'
compile_cmd=['rustc','--edition=2021','-O','--forbid','unsafe_code',str(D/'compat-restore.rs'),'-L','dependency='+str(R/'target/release/deps')]
for n,p in libs.items():compile_cmd+=['--extern',n+'='+p]
compile_cmd+=['-o',str(exe)]
with (D/'compat-rustc.stdout').open('x') as out,(D/'compat-rustc.stderr').open('x') as err:assert subprocess.run(compile_cmd,cwd=R,stdout=out,stderr=err).returncode==0
args=[str(exe),str(copy),before['rules_identity_unchanged'],str(R/'work/t13a65-joint-02/model/business/fault-reference/cpu-final-engine.json')]
with (D/'compat-restore.stdout').open('x') as out,(D/'compat-restore.stderr').open('x') as err:code=subprocess.run(args,cwd=R,stdout=out,stderr=err).returncode
assert code==0
assert inspect(original)==inspect(copy)==243
sha=lambda p:hashlib.sha256(Path(p).read_bytes()).hexdigest()
proof=dict(command=compile_cmd,run=args,exit=code,helper_sha256=sha(D/'compat-restore.rs'),executable_sha256=sha(exe),linked_rlibs={n:dict(path=p,sha256=sha(p)) for n,p in libs.items()},original_files_verified_unchanged=243,copy_complete_retained_at=str(copy.relative_to(R)),source_original=str(original.relative_to(R)),rules_identity_unchanged=before['rules_identity_unchanged'],result=json.loads((D/'compat-restore.stdout').read_text()),scope='Actual old complete world reopen and canonical full byte comparison; no budget or throughput acceptance')
(D/'compat-proof.json').write_text(json.dumps(proof,indent=2)+'\n');print(json.dumps(proof['result'],indent=2))
