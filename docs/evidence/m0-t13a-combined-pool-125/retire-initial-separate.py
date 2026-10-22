from pathlib import Path
import json,hashlib,os,subprocess,time
repo=Path('/Users/frankqin/dev/MyCity');w=Path('/Volumes/HP P900/dev/MyCity/work');old=w/'pool-control-current-01/separate';new=w/'pool-control-current-01/separate-control';e=w/'pool-control-current-01/retirement-initial-separate';e.mkdir(exist_ok=False)
def sha(p):
 h=hashlib.sha256()
 with p.open('rb') as f:
  for b in iter(lambda:f.read(8*1024*1024),b''):h.update(b)
 return h.hexdigest()
proc=subprocess.check_output(['ps','-axo','pid=,comm='],text=True)
for line in proc.splitlines():
 a=line.split(None,1)
 if len(a)==2:assert Path(a[1]).name not in ['mycity-headless','vmmap','cargo','rustc','clippy-driver'],line
(e/'processes-before-comm-only.txt').write_text(proc)
for root in [old,new]:
 v=json.loads((root/'observer-report.json').read_text());assert v['model_exit_code']==0
 check=subprocess.run(['lsof','-nP','+D',str(root)],capture_output=True,text=True);assert check.returncode==1 and not check.stdout and not check.stderr
 report=json.loads((root/'model/layout/report.json').read_text());assert len(report['snapshots'])==2
 assert all(x['complete_record_restore']=='passed' and x['full_pressure_file_readback']=='passed' for x in report['snapshots'])
 assert json.loads((root/'model/exercise-report.json').read_text())['layout_and_live_provider_cold_exit_zero']
protected=json.loads((repo/'docs/evidence/work-migration-20261022/attempt03/protected-after.json').read_text());identities={(p['metadata'][0],p['metadata'][1]) for p in protected};protected_paths={(repo/p['path']).resolve() for p in protected}
# Verify the complete latest recovery baseline before retiring only obsolete synthetic files.
latest=[];report=json.loads((new/'model/layout/report.json').read_text())
for snapshot in report['snapshots']:
 for row in snapshot['files']:
  p=new/f"model/layout/snapshots/snapshot-{snapshot['generation']:06d}"/row['name'];assert p.is_file() and p.stat().st_size==row['bytes'] and sha(p)==row['sha256'];latest.append({'path':str(p),'bytes':row['bytes'],'sha256':row['sha256']})
(e/'latest-complete-baseline-verified.json').write_text(json.dumps(latest,indent=2));files=[];report=json.loads((old/'model/layout/report.json').read_text())
for snapshot in report['snapshots']:
 for row in snapshot['files']:
  if row['name'] not in ['pressure.bin','work-reserve.bin']:continue
  p=old/f"model/layout/snapshots/snapshot-{snapshot['generation']:06d}"/row['name'];s=p.stat();assert not p.is_symlink() and p.resolve() not in protected_paths and (s.st_dev,s.st_ino) not in identities;assert s.st_nlink==1 and s.st_size==row['bytes'] and sha(p)==row['sha256'];files.append({'path':str(p),'bytes':s.st_size,'allocated_bytes':s.st_blocks*512,'device':s.st_dev,'inode':s.st_ino,'mtime_ns':s.st_mtime_ns,'sha256':row['sha256'],'reason':'obsolete synthetic pressure/reserve after actual two-generation and cold-read validation;final-source separate-control retains a fully verified two-generation recovery baseline; initial separate raw evidence and semantic payload retained','retained_evidence':str(old/'observer-report.json')})
assert len(files)==4
(e/'deletion-intent.json').write_text(json.dumps({'files':files,'not_directly_restorable_after_retirement':str(old),'new_complete_baseline':str(new),'dependency_review':'initial separate source4a818 is superseded by the same-source ac02 separate/combined control pair; only its raw diagnostic logs remain referenced; no future restore uses this initial source; final sourceac02 separate-control two complete generations cold-read and every file SHA verified; frozen source/exe/190 prior originals preserved'},indent=2))
# Recheck occupancy and original identity immediately before deletion.
assert subprocess.run(['lsof','-nP','+D',str(old)],capture_output=True,text=True).returncode==1
s=os.statvfs(w);before=s.f_bavail*s.f_frsize
for row in files:
 p=Path(row['path']);s=p.stat();assert (s.st_dev,s.st_ino,s.st_size,s.st_mtime_ns)==(row['device'],row['inode'],row['bytes'],row['mtime_ns']);p.unlink()
s=os.statvfs(w);after=s.f_bavail*s.f_frsize
assert all((repo/p['path']).is_file() for p in protected)
receipt={'deleted_files':files,'allocated_bytes_removed':sum(x['allocated_bytes'] for x in files),'logical_bytes_removed':sum(x['bytes'] for x in files),'available_before_bytes':before,'available_after_bytes':after,'observed_volume_available_delta':after-before,'exclusive_APFS_physical_reclaim':'not_claimed','latest_recovery_baseline_full_files_verified_and_retained':str(new),'original190_paths_retained':True,'old02_status':'initial separate cold recovery passed before retirement; superseded by final-source control; no longer directly restorable after four synthetic components removed','no_source_dependency_executable_log_or_business_archive_deleted':True}
(e/'receipt.json').write_text(json.dumps(receipt,indent=2));(old/'RETENTION.md').write_text('初始separate功能恢复当时通过，已由最终同源对照替代。其两代pressure.bin/work-reserve.bin已精确退役，当前不再可直接完整恢复。所有日志、协议、语义组件及固定源码/exe保留；最终ac02 separate-control两代完整恢复基线逐文件SHA验证后保留。退役收据：'+str(e/'receipt.json')+'\n');print(json.dumps({k:v for k,v in receipt.items() if k!='deleted_files'},indent=2))
