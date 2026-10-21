"""Stage-local cache removal: fixed Debug-only cache, never source/work/release evidence."""
import collections,json,os,shutil,subprocess,time
from pathlib import Path
R=Path('/Users/frankqin/dev/MyCity');D=Path(__file__).resolve().parent;cache=R/'target/debug'
assert not cache.is_symlink()
parts=[cache/'.fingerprint',cache/'build',cache/'deps']
for p in parts: assert p.is_dir() and not p.is_symlink()
# Debug deps hold only reproducible library/metadata outputs, not acceptance executables.
assert all(p.suffix in {'.d','.rmeta','.rlib','.dylib'} for p in (cache/'deps').iterdir() if p.is_file())
raw=subprocess.check_output(['ps','-axo','pid=,ppid=,command='],text=True)
using=[]
for line in raw.splitlines():
 fields=line.split(None,2)
 if len(fields)==3 and str(cache) in fields[2]:using.append({'pid':int(fields[0]),'ppid':int(fields[1]),'uses_debug_path':True})
opened=subprocess.run(['lsof','-nP','+D',str(cache)],text=True,capture_output=True)
assert not using,using
assert opened.returncode==1 and not opened.stdout.strip() and not opened.stderr.strip(), (opened.returncode,opened.stdout,opened.stderr)
files=[p for part in parts for p in part.rglob('*') if p.is_file()]
assert all(not p.is_symlink() for part in parts for p in part.rglob('*'))
stats={}
for p in files:
 s=p.stat();stats[(s.st_dev,s.st_ino)]=s
assert all(s.st_nlink==1 for s in stats.values())
record={'schema':'mycity-stage-cache-cleanup-v1','checked_unix_ns':time.time_ns(),'paths':[str(p.relative_to(R)) for p in parts],
 'process_debug_path_matches':using,'lsof_exit':opened.returncode,'open_debug_files':[],
 'future_validation':'all planned/current cargo commands use --release and shared target/release; debug caches are not dependencies',
 'preserved':['source and WIP','Cargo.lock and frozen source/dependency snapshots','target/release shared future cache and binaries','work candidate/accepted executables','all acceptance logs/receipts/artifacts and unique L1 snapshots'],
 'file_count':len(files),'file_bytes':sum(s.st_size for s in stats.values()),'file_allocated_bytes':sum(s.st_blocks*512 for s in stats.values()),
 'directory_allocated_bytes':sum(p.stat().st_blocks*512 for part in parts for p in [part,*part.rglob('*')] if p.is_dir()),
 'file_suffix_counts':dict(collections.Counter(p.suffix for p in files)), 'free_bytes_before':shutil.disk_usage(R).free}
(D/'cache-check-before.json').write_text(json.dumps(record,indent=2)+'\n')
# Recheck open files immediately before mutation; no intervening builds/tests are launched.
again=subprocess.run(['lsof','-nP','+D',str(cache)],text=True,capture_output=True)
assert again.returncode==1 and not again.stdout.strip() and not again.stderr.strip()
for p in parts: shutil.rmtree(p)
record.update({'deleted_paths_absent':all(not p.exists() for p in parts),'free_bytes_after':shutil.disk_usage(R).free})
record['filesystem_available_delta_bytes']=record['free_bytes_after']-record['free_bytes_before']
record['space_semantics']='removed unique file allocated blocks plus directory blocks; filesystem free delta independently measured and may include unrelated activity/APFS effects'
(D/'cache-check.json').write_text(json.dumps(record,indent=2)+'\n');print(json.dumps(record,indent=2))
