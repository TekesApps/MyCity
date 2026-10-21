"""Finite allowlist: retire only the completed L1 diagnostic synthetic payloads.
Never run while MyCity measurement/build is active. No directory deletion.
"""
from pathlib import Path
import gzip, hashlib, json, os, stat, subprocess, time
R=Path('/Users/frankqin/dev/MyCity');D=Path(__file__).resolve().parent
os.chdir(R)
S=Path('work/native-boundaries-l1-smoke-01/observed/model/layout/snapshots/snapshot-000001')
ALLOWED=[S/'pressure.bin',S/'work-reserve.bin']
PROTECTED=[Path('work/t13a50-joint-01/model/layout/snapshots/snapshot-000001')]
PROTECTED += sorted(Path('work/steady-controller-formal-l2-01/observed/model/layout/snapshots').glob('snapshot-*'))
PROTECTED += sorted(Path('work/steady-controller-smoke-l2-01/observed/model/layout/snapshots').glob('snapshot-*'))
def save(n,v):
 with (D/n).open('w') as f:
  json.dump(v,f,indent=2);f.write('\n');f.flush();os.fsync(f.fileno())
 fd=os.open(D,os.O_RDONLY);os.fsync(fd);os.close(fd)
def sha(p):
 h=hashlib.sha256()
 with p.open('rb') as f:
  for b in iter(lambda:f.read(4*1024**2),b''):h.update(b)
 return h.hexdigest()
def meta(p):
 s=p.lstat();return [s.st_dev,s.st_ino,s.st_size,s.st_mtime_ns,s.st_blocks*512,s.st_nlink]
def inventory():
 return {str(Path(d)/n):meta(Path(d)/n) for d,_,fs in os.walk('work') for n in fs}
def source():
 paths=subprocess.check_output(['git','ls-files','-z','--cached','--others','--exclude-standard'],text=True).split('\0')
 return {p:sha(Path(p)) for p in paths if p and Path(p).is_file() and not p.startswith('docs/') and p!='README.md'}
def metrics():
 s=os.statvfs(R)
 return dict(time_ns=time.time_ns(),work_du_kib=int(subprocess.check_output(['du','-sk','work'],text=True).split()[0]),target_du_kib=int(subprocess.check_output(['du','-sk','target'],text=True).split()[0]),filesystem_available_bytes=s.f_bavail*s.f_frsize)
def idle(label):
 for prior_pid in (86456,):
  try: os.kill(prior_pid,0)
  except ProcessLookupError: pass
  else: raise AssertionError(f'Prior measurement/runner PID still exists: {prior_pid}')
 ps=subprocess.check_output(['ps','-axo','pid,ppid,etime,comm'],text=True);(D/(label+'-processes.txt')).write_text(ps)
 assert not any(x.split()[-1].endswith('mycity-headless') for x in ps.splitlines() if x.split()),'live model'
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
 return hits
def protected():
 rows=[]
 for root in PROTECTED:
  idx=json.loads((root/'index.json').read_text())
  for row in idx['files']:
   p=root/row['name'];assert p.stat().st_size==row['bytes'] and sha(p)==row['sha256'],str(p)
  rows.append(dict(path=str(root),index_sha256=sha(root/'index.json'),indexed_files=len(idx['files']),all_original_bytes_verified=True))
 return rows
assert not (D/'deleted.json').exists(),'one-shot; do not repeat'
assert Path('work/native-boundaries-l1-smoke-01/observed/observer-report.json').exists(),'measurement not terminal'
idle('prepare');save('prepare-metrics.json',metrics());save('source-before.json',source())
(D/'git-before.txt').write_text(subprocess.check_output(['git','status','--short'],text=True))
with gzip.open(D/'work-before.json.gz','wt') as f:json.dump(inventory(),f)
save('protected-before.json',protected())
report=json.loads(Path('work/native-boundaries-l1-smoke-01/observed/model/layout/report.json').read_text())
assert report['diagnostic_outcome']=={'Ok':None} and report['all_published_scans_completed']
assert len(report['snapshots'])==1 and report['snapshots'][0]['pressure_readback_passed'] and report['snapshots'][0]['record_restore_passed']
cold=json.loads(Path('work/native-boundaries-l1-smoke-01/observed/model/layout-restored/report.json').read_text())
assert cold['cold_reopen_and_scan']=='passed'
save('retained-cold-restore-report.json',cold)
save('retained-layout-report.json',report);index=json.loads((S/'index.json').read_text());save('retained-snapshot-index.json',index)
indexed={r['name']:r for r in index['files']};plan=[]
for p in ALLOWED:
 s=p.lstat();r=indexed[p.name]
 assert stat.S_ISREG(s.st_mode) and s.st_nlink==1 and s.st_size==r['bytes'] and sha(p)==r['sha256']
 plan.append(dict(path=str(p),round='native-boundaries-l1-smoke-01',metadata=meta(p),sha256=r['sha256'],reason='Completed shortened-wall L1 diagnostic; full readback and cold record restore passed; no subsequent full-snapshot dependency; regenerable original-load synthetic data, not business',evidence='docs/evidence/m0-t13a-native-boundaries/native-boundaries-l1-smoke-01.stdout; work/native-boundaries-l1-smoke-01/observed/model/layout/{events.jsonl,report.json,snapshots/snapshot-000001/index.json}',directly_restorable_after=False))
# Components may retire only if existing full-byte archive matches exactly.
archive_candidates=[];unmatched=[]
for r in index['files']:
 name=r['name']
 if not name.endswith('.json') or name in ('models.json','preview.json'):continue
 archive=Path('work/retained-component-evidence')/('duplicate-components-v1' if name in ('native.json','meter.json') else 'duplicate-stubs-v1')/(name+'.gz')
 p=S/name
 if not archive.exists():unmatched.append({'path':str(p),'reason':'no existing full byte archive; retain'});continue
 h=hashlib.sha256();length=0;equal=True
 with gzip.open(archive,'rb') as z,p.open('rb') as original:
  for b in iter(lambda:z.read(1024**2),b''):
   length+=len(b);h.update(b)
   if b!=original.read(len(b)):equal=False
  if original.read(1):equal=False
 if not equal or length!=r['bytes'] or h.hexdigest()!=r['sha256']:
  unmatched.append({'path':str(p),'reason':'not equal to existing archive; retain'});continue
 assert stat.S_ISREG(p.lstat().st_mode) and p.lstat().st_nlink==1
 plan.append(dict(path=str(p),round='native-boundaries-l1-smoke-01',metadata=meta(p),sha256=r['sha256'],archive=str(archive),archive_sha256=sha(archive),reason='Completed pure synthetic diagnostic without recovery dependency; existing lossless component archive verified byte-for-byte',evidence='retained original index, cold restore report, actual model logs and existing full-byte gzip archive',directly_restorable_after=False))
save('unmatched-components-retained.json',unmatched)
save('plan.json',plan)
assert source()==json.loads((D/'source-before.json').read_text())
idle('execute');before=metrics();save('delete-before-metrics.json',before)
removed=[]
for row in plan:
 p=Path(row['path']);assert meta(p)==row['metadata'];p.unlink();removed.append(row);save('deleted.json',removed)
fd=os.open(S,os.O_RDONLY);os.fsync(fd);os.close(fd)
after=metrics();save('delete-after-metrics.json',after)
old=json.loads(gzip.open(D/'work-before.json.gz','rt').read());now=inventory();deleted={r['path'] for r in plan}
assert set(old)-set(now)==deleted and not(set(now)-set(old))
allocation=[]
for p,m in old.items():
 if p in deleted:continue
 a=now[p];assert [m[i] for i in (0,1,2,3,5)]==[a[i] for i in (0,1,2,3,5)],p
 if m[4]!=a[4]:allocation.append(dict(path=p,before=m[4],after=a[4]))
save('retained-allocation-only-differences.json',allocation)
save('protected-after.json',protected());assert protected()==json.loads((D/'protected-before.json').read_text())
save('source-after.json',source());assert source()==json.loads((D/'source-before.json').read_text())
save('result.json',dict(before=before,after=after,deleted_files=len(plan),deleted_unique_single_link_inode_allocated_bytes=sum(r['metadata'][4] for r in plan),target_deleted_bytes=0,new_work_archive_bytes=0,retained_files_identity_length_mtime_links_unchanged=len(now),retained_allocation_only_differences=len(allocation),observed_df_available_delta=after['filesystem_available_bytes']-before['filesystem_available_bytes'],df_delta_attributed_to_cleanup=None,availability='Native diagnostic L1 snapshot now incomplete, cannot directly restore; formal7/8, v50 compatibility and smoke-L2 fault full originals preserved',accounting='st_blocks not exclusive APFS physical bytes; df influenced by snapshots/shared blocks and unrelated tasks; old cleanup not counted'))
print(json.dumps(json.loads((D/'result.json').read_text()),indent=2))
