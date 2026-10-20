#!/usr/bin/env python3
"""Compile a Rust reader against Cargo's exact current library artifacts, read v0.5.33."""
import hashlib,json,subprocess,zipfile
from pathlib import Path
D=Path(__file__).parent;R=D.resolve().parents[2];run=R/'work/t13a2-v1-compatibility';run.mkdir()
with zipfile.ZipFile(D.parent/'m0-t13a-storage/world.zip') as z:
 assert all(not Path(n).is_absolute() and '..' not in Path(n).parts for n in z.namelist());z.extractall(run/'world')
 c=json.loads(z.read('CURRENT.json'));r=json.loads(z.read(f'root-{c["slot"]}.json'));cp=json.loads(z.read(f'checkpoint-{r["checkpoint"]["generation"]}.json'))
 receipts=[];ref=r['checkpoint']
 while ref:
  p=json.loads(z.read(f'checkpoint-{ref["generation"]}.json'));receipts+=p['receipts'];ref=p['previous']
 expected={'engine':json.loads(bytes(cp['engine'])),'receipts':receipts};(run/'expected.json').write_text(json.dumps(expected))
def fingerprints():
 return {str(p.relative_to(run/'world')):hashlib.sha256(p.read_bytes()).hexdigest() for p in (run/'world').rglob('*') if p.is_file()}
before=fingerprints()
command=['cargo','build','-p','mycity-storage','--lib','--release','--locked','--offline','--message-format=json']
build=subprocess.run(command,cwd=R,stdout=subprocess.PIPE,stderr=subprocess.PIPE,check=True)
(D/'compatibility-build.jsonl').write_bytes(build.stdout);(D/'compatibility-build-stderr.txt').write_bytes(build.stderr)
artifacts={}
for b in build.stdout.splitlines():
 j=json.loads(b)
 if j.get('reason')=='compiler-artifact':
  for n in j['filenames']:
   if n.endswith('.rlib'):artifacts[j['target']['name']]=n
lib=Path(artifacts['mycity_storage']);serde=Path(artifacts['serde_json']);binary=run/'reader'
compile=['rustc','--edition=2021','--crate-name','mycity_v1_compatibility',str(D/'compatibility.rs'),'--extern','mycity_storage='+str(lib),'--extern','serde_json='+str(serde),'-L','dependency='+str(R/'target/release/deps'),'-O','-o',str(binary)]
subprocess.run(compile,cwd=R,check=True)
result=subprocess.run([str(binary),str(run/'world'),str(run/'expected.json')],cwd=R,stdout=subprocess.PIPE,stderr=subprocess.PIPE,check=True)
assert before==fingerprints()
report=json.loads(result.stdout);report.update({'original_file_bytes_unchanged':True,'storage_rlib_sha256':hashlib.sha256(lib.read_bytes()).hexdigest(),'serde_rlib_sha256':hashlib.sha256(serde.read_bytes()).hexdigest(),'reader_source_sha256':hashlib.sha256((D/'compatibility.rs').read_bytes()).hexdigest(),'reader_binary_sha256':hashlib.sha256(binary.read_bytes()).hexdigest(),'current_source_sha256':json.loads((D/'source.json').read_text())['source_sha256'],'compile_command':compile,'cargo_command':command})
(D/'compatibility.json').write_text(json.dumps(report,indent=2)+'\n');print(json.dumps(report,indent=2))
