import hashlib,json,re,subprocess,tarfile,zipfile
from pathlib import Path
R=Path('/Users/frankqin/dev/MyCity');D=Path(__file__).resolve().parent
# Cargo's generated normalized lockfile: compare unchanged package blocks exactly.
def packages(text):
 out={}
 for block in text.split('[[package]]')[1:]:
  field=lambda key:json.loads(re.search(r'^'+key+r' = (".*")$',block,re.M)[1])
  name,version=field('name'),field('version');out[(name,version)]={'name':name,'version':version,'block':block.strip(),'checksum':field('checksum') if 'checksum = ' in block else None}
 return out
old=packages(subprocess.check_output(['git','show','v0.5.63:Cargo.lock'],text=True));new=packages((R/'Cargo.lock').read_text());assert old.keys()<=new.keys()
for key,p in old.items():
 q=new[key];assert p['checksum']==q['checksum']
 if key[0]!='sha2':assert p['block']==q['block'],key
 else:assert p['block'].replace(' "digest",',' "digest",\n "sha2-asm",')==q['block']
added=[p for k,p in new.items() if k not in old];proof=[]
with zipfile.ZipFile(D/'new-dependency-sources.zip','x',zipfile.ZIP_DEFLATED) as z:
 for p in added:
  name=p['name']+'-'+p['version']+'.crate';candidates=list(Path('/Users/frankqin/.cargo/registry/cache').glob('*/'+name));assert candidates,name
  data=candidates[0].read_bytes();h=hashlib.sha256(data).hexdigest();assert h==p['checksum'];z.writestr(name,data)
  with tarfile.open(candidates[0]) as tf:
   licenses=[x.name for x in tf.getmembers() if x.isfile() and ('LICENSE' in x.name.upper() or 'COPYING' in x.name.upper())];assert licenses,name
   meta=tf.extractfile(p['name']+'-'+p['version']+'/Cargo.toml').read().decode();license_id=json.loads(re.search(r'^license = (".*")$',meta,re.M)[1])
   proof.append({'name':p['name'],'version':p['version'],'registry_checksum':h,'archive':name,'license':license_id,'license_files':licenses})
(D/'dependency-proof.json').write_text(json.dumps({'old_package_blocks_and_checksums_unchanged':True,'only_old_dependency_edge_added':'sha2 -> sha2-asm','new_packages':proof,'target_condition':'aarch64','unsafe_lint':'unchanged; dependencies contain unsafe/assembly','other_platform_builds':'not_run','soft_fallback_execution':'not_run'},indent=2)+'\n')
print(json.dumps(proof,indent=2))
