#!/usr/bin/env python3
import hashlib,json,shutil,subprocess,time
from pathlib import Path
D=Path(__file__).resolve().parent;R=D.parents[2]
assert all(x['exit']==0 for x in json.loads((D/'checks-final.json').read_text()).values())
proof=json.loads((D/'compat-proof.json').read_text());original=R/proof['source_original']
fork=R/'work/t13a73-checkpoint-roundtrip/world';assert not fork.exists()
fork.parent.mkdir(parents=True);shutil.copytree(original,fork)
libs={}
for line in (D/'compat-build-artifacts.jsonl').read_text().splitlines():
 x=json.loads(line)
 if x.get('reason')=='compiler-artifact' and x['target']['name'] in ['mycity_core','mycity_storage','serde_json']:
  libs[x['target']['name']]=next(f for f in x['filenames'] if f.endswith('.rlib'))
assert len(libs)==3
exe=R/'work/candidate-binaries/store-stream/roundtrip'
cmd=['rustc','--edition=2021','-O','--forbid','unsafe_code',str(D/'roundtrip.rs'),'-L','dependency='+str(R/'target/release/deps')]
for n,p in libs.items():cmd+=['--extern',n+'='+p]
cmd+=['-o',str(exe)]
with (D/'roundtrip-rustc.stdout').open('x') as out,(D/'roundtrip-rustc.stderr').open('x') as err:
 assert subprocess.run(cmd,cwd=R,stdout=out,stderr=err).returncode==0
args=[str(exe),str(fork),proof['rules_identity_unchanged'],str(R/'work/t13a65-joint-02/model/business/fault-reference/cpu-final-engine.json')]
with (D/'roundtrip.stdout').open('x') as out,(D/'roundtrip.stderr').open('x') as err:
 code=subprocess.run(args,cwd=R,stdout=out,stderr=err).returncode
result=dict(command=cmd,run=args,exit=code,executable_sha256=hashlib.sha256(exe.read_bytes()).hexdigest(),world=str(fork.relative_to(R)),result=json.loads((D/'roundtrip.stdout').read_text()) if code==0 else None,scope='Correctness diagnostic of full old world and actual new streaming checkpoint; no performance/load gate accepted')
(D/'roundtrip-proof.json').write_text(json.dumps(result,indent=2)+'\n')
assert code==0;print(json.dumps(result['result'],indent=2))
