"""One-shot exact retirement; no directory-prefix deletion, no model/build execution."""
import gzip,hashlib,json,os,shutil,stat,subprocess,sys,time
from pathlib import Path
R=Path('/Users/frankqin/dev/MyCity');D=Path(__file__).resolve().parent
os.chdir(R)
def save(n,v):
 with (D/n).open('w') as f:json.dump(v,f,indent=2);f.write('\n');f.flush();os.fsync(f.fileno())
def sha(p):
 h=hashlib.sha256()
 with Path(p).open('rb') as f:
  for b in iter(lambda:f.read(1024*1024),b''):h.update(b)
 return h.hexdigest()
def meta(p):
 s=Path(p).lstat();return [s.st_dev,s.st_ino,s.st_mode,s.st_size,s.st_mtime_ns,s.st_blocks,s.st_nlink]
def inv():
 return {str(Path(a)/n):meta(Path(a)/n) for root in ['work','target'] for a,_,files in os.walk(root) for n in files}
def wip():
 names=subprocess.check_output(['git','ls-files','--cached','--others','--exclude-standard'],text=True).splitlines()
 return {p:sha(p) for p in names if Path(p).is_file() and (p.startswith('crates/') or p in ['Cargo.toml','Cargo.lock'])}
def occupancy(label):
 ps=subprocess.check_output(['ps','-axo','pid,ppid,etime,command'],text=True)
 rows=[];active=[]
 for l in ps.splitlines()[1:]:
  f=l.split(None,3)
  if len(f)!=4:continue
  exe=f[3].split()[0];rows.append(' '.join(f[:3]+[exe]))
  if any(s in Path(exe).name.lower() for s in ['mycity','cargo','rustc']):active.append(l)
 (D/(label+'-processes.txt')).write_text('\n'.join(rows)+'\n')
 external=[]
 for line in active:
  pid=line.split()[0]
  q=subprocess.run(['lsof','-a','-p',pid,'-d','cwd','-Fn'],capture_output=True,text=True)
  dirs=[l[1:] for l in q.stdout.splitlines() if l.startswith('n')]
  if not dirs:
   live=subprocess.run(['ps','-p',pid,'-o','pid='],capture_output=True,text=True)
   assert not live.stdout.strip(),('active process cwd unknown',pid)
   continue
  assert all(Path(d)!=R and R not in Path(d).parents for d in dirs),('project process active',pid,dirs)
  external.append(dict(pid=int(pid),cwd=dirs,action='unrelated project; not stopped or cleaned'))
 save(label+'-external-builds.json',external)
 p=subprocess.run(['lsof','-nP','+D',str(R/'work'),'+D',str(R/'target')],capture_output=True,text=True)
 save(label+'-occupancy.json',dict(code=p.returncode,stdout=p.stdout,stderr=p.stderr))
 assert p.returncode==1 and not p.stdout and not p.stderr,'occupied or incomplete lsof'
 return dict(time_ns=time.time_ns(),du_kib={p:int(subprocess.check_output(['du','-sk',p],text=True).split()[0]) for p in ['work','target']},available_bytes=shutil.disk_usage(R).free)
def protected():
 paths={x['path'] for x in json.loads(Path('docs/evidence/disk-reclamation-v101/protected-after.json').read_text())}
 p=Path('work/r07-report-error-l1-01/observed/model/layout/snapshots/snapshot-000002')
 paths.update(str(x) for x in p.iterdir() if x.is_file())
 return [dict(path=p,metadata=meta(p),sha256=sha(p)) for p in sorted(paths)]
def identity(m):return m[:5]+m[6:]
def block_changes(before,after):
 assert before.keys()==after.keys(),'file set changed'
 out=[]
 for p,b in before.items():
  a=after[p];assert identity(b)==identity(a),('identity/content metadata changed',p)
  if a[5]!=b[5]:out.append(dict(path=p,before_blocks=b[5],after_blocks=a[5],allocated_delta_bytes=(a[5]-b[5])*512,attribution='retained allocation metadata fluctuation, excluded from deleted-file total'))
 return out
def check_protected(rows):
 for x in rows:assert identity(meta(x['path']))==identity(x['metadata']) and sha(x['path'])==x['sha256'],x['path']
if sys.argv[1]=='prepare':
 assert not (D/'plan.json').exists()
 E=D.parent
 terminal=json.loads((E/'regression-terminal.json').read_text());assert terminal['all_commands_exit_zero']
 current=json.loads((E/'exercise-result.json').read_text());assert current['exit']==0 and current['independent_cold']['exit']==0
 assert current['second_snapshot_r07']['replaced_bytes']>0
 for p in ['work/r07-report-error-l1-01/independent-cold/report.json','work/r07-report-error-compat-01/v3-predecessor/report.json']:
  assert json.loads(Path(p).read_text())['cold_reopen_and_scan']=='passed'
 before=occupancy('prepare');inventory=inv();plan=[]
 rounds=[('r07-coupling-l1-01',1),('r07-event-driven-l1-01',1),('r07-event-driven-l1-01',2),('r07-report-error-l1-01',1)]
 for run,gen in rounds:
  root=Path('work')/run/'observed/model/layout/snapshots'/f'snapshot-{gen:06}'
  index=json.loads((root/'index.json').read_text());assert index['schema']=='mycity-layout-snapshot-v3'
  for name in ['pressure.bin','work-reserve.bin']:
   p=root/name;m=meta(p);h=sha(p);assert stat.S_ISREG(m[2]) and m[6]==1
   e=next(e for e in index['files'] if e['name']==name);assert e['bytes']==m[3] and e['sha256']==h
   plan.append(dict(path=str(p),metadata=m,sha256=h,allocated_bytes=m[5]*512,round=run,generation=gen,reason='terminal synthetic payload; actual full readback/cold validation and latest full v3 generation2 successor released dependency; snapshot_busy was diagnostic scheduling, not payload fault',retained_evidence='original index/components/raw/report/source/exe; docs/evidence/m0-t13a-r07-report-error; original coupling snapshot_busy evidence retained',index=str(root/'index.json'),index_sha256=sha(root/'index.json'),directly_restorable_after=False))
 with gzip.open(D/'inventory-before.json.gz','wt') as f:json.dump(inventory,f,separators=(',',':'))
 save('wip-before.json',wip());save('protected-before.json',protected());save('plan.json',plan);save('prepare.json',before)
 print(json.dumps(dict(files=len(plan),allocated_bytes=sum(x['allocated_bytes'] for x in plan),before=before),indent=2))
elif sys.argv[1]=='refresh':
 assert (D/'preflight-inventory-difference.json').is_file() and not (D/'delete-intent.json').exists()
 old=json.loads(gzip.open(D/'inventory-before.json.gz','rt').read());new=inv();delta=block_changes(old,new)
 pr=json.loads((D/'protected-before.json').read_text());check_protected(pr);assert wip()==json.loads((D/'wip-before.json').read_text())
 for x in json.loads((D/'plan.json').read_text()):assert Path(x['path']).exists()
 obs=occupancy('refresh');save('preflight-failure.json',dict(deleted_files=0,reason='st_blocks changed only; all identities and protected content SHA reverified',changes=delta))
 (D/'inventory-before.json.gz').rename(D/'inventory-preflight-original.json.gz')
 with gzip.open(D/'inventory-before.json.gz','wt') as f:json.dump(new,f,separators=(',',':'))
 save('refresh.json',obs)
elif sys.argv[1]=='execute':
 assert (D/'advisor-review.md').is_file() and not (D/'result.json').exists()
 plan=json.loads((D/'plan.json').read_text());inventory=json.loads(gzip.open(D/'inventory-before.json.gz','rt').read());pr=json.loads((D/'protected-before.json').read_text());wp=json.loads((D/'wip-before.json').read_text())
 changes_before=block_changes(inventory,inv());assert wip()==wp;check_protected(pr)
 for x in plan:
  assert identity(meta(x['path']))==identity(x['metadata']) and sha(x['path'])==x['sha256'];assert sha(x['index'])==x['index_sha256']
 before=occupancy('immediate');save('delete-intent.json',plan)
 for x in plan:
  p=Path(x['path']);m=meta(p);assert identity(m)==identity(x['metadata']);x['metadata_at_unlink']=m;x['allocated_bytes']=m[5]*512;p.unlink()
 after=occupancy('after');post=inv();gone={x['path'] for x in plan}
 changes_after=block_changes({p:v for p,v in inventory.items() if p not in gone},post);save('retained-block-changes.json',dict(before_delete=changes_before,after_delete=changes_after,excluded_from_reclamation=True))
 assert wip()==wp;check_protected(pr);save('protected-after.json',[dict(x,metadata=meta(x['path']))for x in pr]);save('deleted.json',plan)
 save('availability.json',dict(retired_snapshots_not_directly_restorable=[{'round':r,'generation':g}for r,g in [('r07-coupling-l1-01',1),('r07-event-driven-l1-01',1),('r07-event-driven-l1-01',2),('r07-report-error-l1-01',1)]],complete_v3_successor='work/r07-report-error-l1-01/observed/model/layout/snapshots/snapshot-000002',five_old_complete_or_fault_originals_retained=True,pending_not_valid_restore_point=True))
 result=dict(before=before,after=after,files_deleted=len(plan),allocated_bytes_removed=sum(x['allocated_bytes'] for x in plan),filesystem_available_delta_bytes=after['available_bytes']-before['available_bytes'],other_work_target_files_identity_metadata_unchanged=len(post),retained_block_metadata_changes_excluded=True,protected_files_sha_verified=len(pr),source_and_wip_unchanged=True,new_archive_bytes=0,build_or_model_run=False,attribution='unique inode st_blocks, not APFS exclusive physical claim; filesystem delta observational only')
 save('result.json',result);print(json.dumps(result,indent=2))
