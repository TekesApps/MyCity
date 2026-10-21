"""Build a synthetic near-limit v1 checkpoint from the retained complete small v1 world.
All base bytes preserved; only synthetic copies under parser-inputs are disposable.
"""
from pathlib import Path
import json,hashlib,subprocess,shutil,os
D=Path(__file__).resolve().parent;R=D.parents[2];os.chdir(R)
def sha(p):return hashlib.sha256(Path(p).read_bytes()).hexdigest()
def encode(v):return json.dumps(v,separators=(',',':')).encode()
def save(n,v):(D/n).write_text(json.dumps(v,indent=2)+'\n')
assert (D/'parser-heap-results.json').exists()
base=R/'work/t13a-role-metadata-v1-proof';root=R/'work/t13a-lexical-parser-inputs/store-escaped-key';assert not root.exists()
base_manifest={str(p.relative_to(base)):sha(p) for p in base.rglob('*') if p.is_file()}
shutil.copytree(base/'world',root)
current=json.loads((root/'CURRENT.json').read_bytes());rf=root/f"root-{current['slot']}.json";rv=json.loads(rf.read_bytes())
assert rv.get('checkpoint_layout',0)==0
assert rv['checkpoint']['generation']==3 and rv['sequence']==2
cp=root/'checkpoint-3.json';data=cp.read_bytes();marker=b'"receipts":[{';assert data.count(marker)==1
n=(16*1024*1024-len(data)-len(b'"":0,'))//6
modified=data.replace(marker,marker+b'"'+b'\\u0061'*n+b'":0,',1)
assert len(modified)<=16*1024*1024 and len(modified)>16*1024*1024-6
cp.write_bytes(modified)
index=root/'checkpoint-index-3.json';iv=json.loads(index.read_bytes());iv['data_sha256']=sha(cp);index.write_bytes(encode(iv))
rv['checkpoint']['sha256']=sha(index);rf.write_bytes(encode(rv));current['root_sha256']=sha(rf);(root/'CURRENT.json').write_bytes(encode(current))
# Remove the alternate synthetic root so a corrupt CP could not appear to pass via WAL fallback.
(root/f"root-{1-current['slot']}.json").unlink()
manifest={str(p.relative_to(root)):dict(bytes=p.stat().st_size,sha256=sha(p)) for p in root.rglob('*') if p.is_file()}
save('store-boundary-input.json',dict(base=str(base.relative_to(R)),base_sha256=base_manifest,synthetic_world=str(root.relative_to(R)),checkpoint_bytes=len(modified),only_selected_root=True,files=manifest,not_a_new_business_save=True,retirement='After old/new actual cold-open probes terminal and files unchanged; base v1 originals retained. Synthetic copy has no downstream recovery dependency.'))
libs={}
for line in (D/'metadata-build-final.stdout').read_text().splitlines():
 x=json.loads(line)
 if x.get('reason')=='compiler-artifact' and x['target']['name'] in ['mycity_core','mycity_storage','mycity_map','peak_alloc']:
  libs[x['target']['name']]=next(n for n in x['filenames'] if n.endswith('.rlib'))
assert len(libs)==4
helper=R/'work/candidate-binaries/role-workspace-lexical/store-boundary-heap';assert not helper.exists()
cmd=['rustc','--edition=2021','-O',str(D/'store-boundary-heap.rs'),'-L','dependency='+str(R/'target/release/deps')]
for n,p in libs.items():cmd+=['--extern',n+'='+p]
cmd+=['-o',str(helper)]
with (D/'store-boundary-compile.stdout').open('x') as o,(D/'store-boundary-compile.stderr').open('x') as e:subprocess.run(cmd,stdout=o,stderr=e,check=True)
old=R/'work/candidate-binaries/role-workspace-metadata/metadata-compat'
for name,cmd in [('store-boundary-old-reader',[str(old),str(root),'metadata-compat-proof-v1',str(base/'expected-engine.json')]),('store-boundary-fixed-reader',[str(helper),str(root),str(base/'expected-engine.json')])]:
 with (D/(name+'.stdout')).open('x') as o,(D/(name+'.stderr')).open('x') as e:subprocess.run(cmd,stdout=o,stderr=e,check=True)
for name,v in manifest.items():assert sha(root/name)==v['sha256']
assert base_manifest=={str(p.relative_to(base)):sha(p) for p in base.rglob('*') if p.is_file()}
result=json.loads((D/'store-boundary-fixed-reader.stdout').read_text())
assert result['lexical_capacity']==1024*1024 and result['backings_unchanged'] and result['full_engine_equal']
save('store-boundary-proof.json',dict(result=result,old_reader_sha256=sha(old),new_reader_sha256=sha(helper),helper_source_sha256=sha(D/'store-boundary-heap.rs'),linked_libraries={n:dict(path=p,sha256=sha(p)) for n,p in libs.items()},synthetic_world_and_base_unchanged=True,scope='Actual normal-prepared Store old/new legacy open of a near-16MiB synthetic CP; old/new full Engine and sequence equal, no fallback root. New process requested heap includes entire construction/open lifetime, no reset. Does not grant normal64/common8 or physical memory acceptance.'))
print('Actual near-limit legacy Store old/new open terminal0',flush=True)
