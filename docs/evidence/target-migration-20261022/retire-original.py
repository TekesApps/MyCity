from pathlib import Path
import json,subprocess,shutil,os,collections,hashlib
D=Path('/Users/frankqin/dev/MyCity/docs/evidence/target-migration-20261022');s=(D/'migrate.py').read_text();exec(s[:s.index('assert S.is_dir()')])
assert json.loads((D/'cargo-test.json').read_text())['exit']==0
reuse=json.loads((D/'cargo-reuse.json').read_text());assert reuse['exit']==0 and reuse['core_fresh'] and not reuse['compiled']
assert S.is_symlink() and S.resolve()==T and B.is_dir() and not B.is_symlink()
original=json.loads((D/'source-manifest.json').read_text());assert manifest(B)==original
for label,p in [('original',B),('destination',T)]:
 q=subprocess.run(['lsof','-nP','+D',str(p)],capture_output=True,text=True);save('occupancy-retire-'+label+'.json',dict(code=q.returncode,stdout=q.stdout,stderr=q.stderr));assert q.returncode==1 and not q.stdout and not q.stderr
# Preserve every pre-existing external hardlink, notably fixed acceptance executables.
groups={}
for p,x in original.items():
 if 'sha256' in x:groups.setdefault((x['dev'],x['ino']),[]).append(p)
external=[];claimed=0;alloc=0
for ps in groups.values():
 x=original[ps[0]];alloc+=x['blocks']*512
 if x['nlink']==len(ps):claimed+=x['blocks']*512
 else:external.append(dict(paths=ps,external_links=x['nlink']-len(ps),allocated_bytes=x['blocks']*512,sha256=x['sha256']))
fixed=[];extinos={k for k,ps in groups.items() if original[ps[0]]['nlink']>len(ps)}
for a,_,fs in os.walk(R/'work/candidate-binaries'):
 for n in fs:
  p=Path(a)/n;st=p.stat()
  if (st.st_dev,st.st_ino) in extinos:
   h=hashlib.sha256(p.read_bytes()).hexdigest();fixed.append(dict(path=str(p),sha256=h))
assert len({(Path(x['path']).stat().st_dev,Path(x['path']).stat().st_ino) for x in fixed})==len(external), 'unknown external hardlink retained: investigate before unlink'
save('fixed-original-links-retained.json',fixed);before=free();save('retirement-intent.json',dict(exact_root=str(B),manifest='source-manifest.json',file_count=sum(len(ps) for ps in groups.values()),unique_allocated_bytes=alloc,allocated_bytes_with_no_external_links=claimed,external_links_retained=external,free_before=before))
shutil.rmtree(B)
assert not B.exists() and S.is_symlink() and S.resolve()==T
for x in fixed:assert hashlib.sha256(Path(x['path']).read_bytes()).hexdigest()==x['sha256']
after=free();save('retirement-result.json',dict(original_removed=True,shared_target=str(T),link=str(S),all_copy_bytes_preverified=True,fixed_executables_unchanged=len(fixed),unique_source_allocation_bytes=alloc,allocation_bytes_no_external_links=claimed,retained_external_link_allocation_bytes=alloc-claimed,free_before=before,free_after=after,free_delta_observational={k:after[k]-before[k] for k in before},physical_APFS_exclusive_reclaim='not_claimed'))
print((D/'retirement-result.json').read_text())
