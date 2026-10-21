"""Precise v70 retired synthetic component cleanup; prepare then execute.
Never deletes a complete recovery input, real business, executable, or target.
"""
from pathlib import Path
import gzip, hashlib, json, os, stat, subprocess, sys, time
R=Path('/Users/frankqin/dev/MyCity'); D=Path(__file__).resolve().parent
os.chdir(R)
NAMES=('native','meter','maps','households','housing','visitors','operators','entities','equity_links','general_contracts','od','path_refs','cargo_batches','passenger_batches','bus_train_vehicles','ship_aircraft_vehicles','car_vehicles','matters','team_members','analysis_tasks','hosts','teaching_batches','current_services','cases','bills')
ROOT=Path('work/t13a70-joint-01/model/layout/snapshots')
PROTECTED=[Path('work/t13a85-joint-01/model/layout/snapshots')/f'snapshot-{g:06}' for g in (1,2)]+[Path('work/t13a50-joint-01/model/layout/snapshots/snapshot-000001')]
def syncdir(p):
 fd=os.open(p,os.O_RDONLY)
 try: os.fsync(fd)
 finally: os.close(fd)
def save(n,v):
 with (D/n).open('w') as f:
  json.dump(v,f,indent=2);f.write('\n');f.flush();os.fsync(f.fileno())
 syncdir(D)
def sha(p):
 h=hashlib.sha256()
 with Path(p).open('rb') as f:
  for b in iter(lambda:f.read(4*1024**2),b''):h.update(b)
 return h.hexdigest()
def meta(p):
 s=Path(p).lstat();return [s.st_dev,s.st_ino,s.st_size,s.st_mtime_ns,s.st_blocks*512,s.st_nlink]
def inventory():
 return {str(Path(d)/n):meta(Path(d)/n) for d,_,fs in os.walk('work') for n in fs}
def metrics():
 s=os.statvfs(R)
 return dict(time_ns=time.time_ns(),work_du_kib=int(subprocess.check_output(['du','-sk','work'],text=True).split()[0]),target_du_kib=int(subprocess.check_output(['du','-sk','target'],text=True).split()[0]),filesystem_available_bytes=s.f_bavail*s.f_frsize)
def idle(label):
 ps=subprocess.check_output(['ps','-axo','pid,ppid,lstart,comm'],text=True)
 (D/(label+'-processes.txt')).write_text(ps)
 p=subprocess.run(['lsof','-nP','-Fpcfn'],capture_output=True,text=True);assert p.returncode==0,p.stderr
 hits=[];pid=cmd=fd=''
 for line in p.stdout.splitlines():
  if line.startswith('p'):pid=line[1:]
  elif line.startswith('c'):cmd=line[1:]
  elif line.startswith('f'):fd=line[1:]
  elif line.startswith('n') and line[1:].startswith(str(R)):
   hits.append(dict(pid=pid,command=cmd,fd=fd,path=line[1:]))
 save(label+'-occupancy.json',hits)
 assert not any(h['path'].startswith((str(R/'work'),str(R/'target'))) for h in hits),hits
 assert not any(x.split()[-1].endswith('mycity-headless') for x in ps.splitlines() if x.split())
def source():
 paths=subprocess.check_output(['git','ls-files','-z','--cached','--others','--exclude-standard'],text=True).split('\0')
 return {p:sha(p) for p in paths if p and Path(p).is_file() and not p.startswith('docs/') and p!='README.md'}
def protected():
 out=[]
 for root in PROTECTED:
  idx=json.loads((root/'index.json').read_text())
  for row in idx['files']:
   p=root/row['name']; assert p.stat().st_size==row['bytes'] and sha(p)==row['sha256'],str(p)
  out.append(dict(path=str(root),index_sha256=sha(root/'index.json'),indexed_files=len(idx['files']),full_original_bytes_verified=True))
 return out
def checkrow(row):
 p=Path(row['path']);a=Path(row['archive'])
 assert stat.S_ISREG(p.lstat().st_mode) and meta(p)==row['metadata'] and meta(p)[5]==1
 assert sha(p)==row['sha256'] and sha(a)==row['archive_sha256']
 h=hashlib.sha256();length=0
 with gzip.open(a,'rb') as z,p.open('rb') as original:
  for b in iter(lambda:z.read(1024**2),b''):
   assert b==original.read(len(b));length+=len(b);h.update(b)
  assert original.read(1)==b''
 assert length==row['bytes'] and h.hexdigest()==row['sha256']
if sys.argv[1]=='prepare':
 assert not (D/'plan.json').exists()
 idle('prepare');save('prepare-metrics.json',metrics());save('source-before.json',source())
 (D/'git-before.txt').write_text(subprocess.check_output(['git','status','--short'],text=True))
 with gzip.open(D/'work-before.json.gz','wt') as f:json.dump(inventory(),f)
 save('protected-before.json',protected())
 plan=[]
 for g in (1,2):
  root=ROOT/f'snapshot-{g:06}';assert not (root/'pressure.bin').exists() and not (root/'work-reserve.bin').exists()
  idx=json.loads((root/'index.json').read_text()); indexed={r['name']:r for r in idx['files']}
  for name in NAMES:
   n=name+'.json';p=root/n;a=Path('work/retained-component-evidence')/('duplicate-components-v1' if name in ('native','meter') else 'duplicate-stubs-v1')/(n+'.gz')
   r=indexed[n];row=dict(path=str(p),round='t13a70-joint-01',generation=g,metadata=meta(p),bytes=r['bytes'],sha256=r['sha256'],index=str(root/'index.json'),index_sha256=sha(root/'index.json'),archive=str(a),archive_sha256=sha(a),reason='Retired synthetic L1 normal snapshot superseded by actual v85 full recovery; existing lossless archive verified byte-for-byte; not real business',directly_restorable_after=False)
   checkrow(row);plan.append(row)
 assert len(plan)==50 and len({tuple(r['metadata'][:2]) for r in plan})==50
 save('plan.json',plan);print('PREPARED',len(plan),sum(r['metadata'][4] for r in plan))
elif sys.argv[1]=='execute':
 assert not (D/'deleted.json').exists()
 plan=json.loads((D/'plan.json').read_text())
 assert source()==json.loads((D/'source-before.json').read_text())
 for row in plan:checkrow(row)
 assert protected()==json.loads((D/'protected-before.json').read_text())
 idle('execute');before=metrics();save('delete-before-metrics.json',before)
 for row in plan:
  p=Path(row['path']);assert meta(p)==row['metadata'];p.unlink()
 for root in {str(Path(r['path']).parent) for r in plan}:syncdir(root)
 save('deleted.json',plan);after=metrics();save('delete-after-metrics.json',after)
 old=json.loads(gzip.open(D/'work-before.json.gz','rt').read());now=inventory();deleted={r['path'] for r in plan}
 assert set(old)-set(now)==deleted and not(set(now)-set(old))
 allocation=[]
 for p,m in old.items():
  if p in deleted:continue
  a=now[p];assert [m[i] for i in (0,1,2,3,5)]==[a[i] for i in (0,1,2,3,5)],p
  if m[4]!=a[4]:allocation.append(dict(path=p,before_allocated=m[4],after_allocated=a[4]))
 save('retained-allocation-only-differences.json',allocation)
 save('protected-after.json',protected())
 assert json.loads((D/'protected-after.json').read_text())==json.loads((D/'protected-before.json').read_text())
 save('source-after.json',source())
 assert source()==json.loads((D/'source-before.json').read_text())
 save('result.json',dict(before=before,after=after,deleted_files=len(plan),deleted_single_link_inode_allocated_bytes=sum(r['metadata'][4] for r in plan),new_work_archive_bytes=0,net_file_allocation_removed_bytes=sum(r['metadata'][4] for r in plan),target_deleted_bytes=0,retained_files_identity_length_mtime_links_unchanged=len(now),retained_allocation_only_differences=len(allocation),observed_df_available_delta=after['filesystem_available_bytes']-before['filesystem_available_bytes'],df_delta_attributable_to_cleanup=None,availability='v70 old layouts already incomplete; now 50 original component paths retired too. Existing full byte gzip archives retained. v85 two full generations plus v50 full old-format original unchanged.',accounting='st_blocks allocation is not exclusive APFS physical storage. No other task or historical reclaim counted.'))
 print(json.dumps(json.loads((D/'result.json').read_text()),indent=2))
else:raise SystemExit('prepare|execute')
