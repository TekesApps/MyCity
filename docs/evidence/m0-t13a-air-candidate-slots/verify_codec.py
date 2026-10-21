"""Actual full retained old world: complete independent old Value byte oracle."""
from pathlib import Path
import hashlib,json,subprocess
D=Path(__file__).resolve().parent;R=D.parents[2]
p=json.loads((D/'compat-proof.json').read_text());libs={}
manifest=json.loads((R/'docs/evidence/m0-t13a-model-workspace/legacy-input-manifest.json').read_text())
def verify_inputs():
 for name in ['source_original','copy_complete_retained_at']:
  root=R/p[name]; files={f.relative_to(root).as_posix():f for f in root.rglob('*') if f.is_file()}
  assert set(files)==set(manifest)
  for n,f in files.items():
   data=f.read_bytes();assert len(data)==manifest[n]['bytes'] and hashlib.sha256(data).hexdigest()==manifest[n]['sha256'],str(f)
verify_inputs()
for line in (D/'compat-build-artifacts.jsonl').read_text().splitlines():
 x=json.loads(line)
 if x.get('reason')=='compiler-artifact' and x['target']['name'] in ['mycity_core','mycity_storage','serde_json']:
  libs[x['target']['name']]=next(f for f in x['filenames'] if f.endswith('.rlib'))
assert len(libs)==3
exe=R/'work/candidate-binaries/air-candidate-slots/codec-proof'
cmd=['rustc','--edition=2021','-O','--forbid','unsafe_code',str(D/'codec-proof.rs'),'-L','dependency='+str(R/'target/release/deps')]
for n,f in libs.items():cmd+=['--extern',n+'='+f]
cmd+=['-o',str(exe)]
with (D/'codec-rustc.stdout').open('x') as out,(D/'codec-rustc.stderr').open('x') as err:assert subprocess.run(cmd,cwd=R,stdout=out,stderr=err).returncode==0
args=[str(exe),str(R/p['copy_complete_retained_at']),p['rules_identity_unchanged'],str(R/'work/t13a65-joint-02/model/business/fault-reference/cpu-final-engine.json')]
with (D/'codec.stdout').open('x') as out,(D/'codec.stderr').open('x') as err:code=subprocess.run(args,cwd=R,stdout=out,stderr=err).returncode
assert code==0
verify_inputs()
result=json.loads((D/'codec.stdout').read_text());sha=lambda f:hashlib.sha256(Path(f).read_bytes()).hexdigest()
proof=dict(command=cmd,run=args,exit=code,result=result,helper_sha256=sha(D/'codec-proof.rs'),executable_sha256=sha(exe),libraries={n:dict(path=f,sha256=sha(f)) for n,f in libs.items()})
(D/'codec-proof.json').write_text(json.dumps(proof,indent=2)+'\n');print(json.dumps(result,indent=2))
