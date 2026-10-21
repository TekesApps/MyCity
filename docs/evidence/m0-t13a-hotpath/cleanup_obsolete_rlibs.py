"""Remove only unused workspace static build libraries; keep all executable evidence."""
import hashlib,json,shutil,subprocess,time
from pathlib import Path
R=Path('/Users/frankqin/dev/MyCity');D=Path(__file__).resolve().parent
assert not (D/'cache-cleanup-initial.json').exists(), 'one-shot evidence procedure already executed; re-audit a fresh graph before any future deletion'
rows=[json.loads(l) for l in (D/'build-artifacts.jsonl').open()];used={Path(n).resolve() for r in rows if r['reason']=='compiler-artifact' for n in r['filenames']}
used |= {p.with_suffix('.rlib') for p in list(used) if p.suffix=='.rmeta'}
assert all(r['fresh'] for r in rows if r['reason']=='compiler-artifact')
candidates=[]
for p in (R/'target/release/deps').glob('libmycity_*.rlib'):
 if p.resolve() not in used:
  s=p.stat();candidates.append({'path':str(p.relative_to(R)),'bytes':s.st_size,'allocated':s.st_blocks*512,'links':s.st_nlink})
selected=[]
for x in candidates:
 p=R/x['path'];assert p.parent==R/'target/release/deps' and p.name.startswith('libmycity_') and p.suffix=='.rlib'
 assert p.resolve() not in used and not p.is_symlink();s=p.stat();assert s.st_nlink==1 and s.st_size==x['bytes'] and s.st_blocks*512==x['allocated']
 selected.append(p)
raw=subprocess.check_output(['ps','-axo','pid=,ppid=,command='],text=True)
active=[]
for line in raw.splitlines():
 if str(R/'target') in line:
  f=line.split(None,2);active.append({'pid':int(f[0]),'ppid':int(f[1]),'exe':f[2].split()[0]})
opened=subprocess.run(['lsof','-nP','+D',str(R/'target')],text=True,capture_output=True)
assert not active,active
assert opened.returncode==1 and not opened.stdout.strip() and not opened.stderr.strip()
record={'scope':'only obsolete libmycity_*.rlib absent from current all-target Cargo graph; no executable, source, frozen dependency or receipt removed','observed_unix_ns':time.time_ns(),'deleted':candidates,'active_processes':active,'lsof_exit':opened.returncode,'current_all_target_artifacts':len(used),'current_build_was_entirely_fresh':True,'allocated_bytes_removed':sum(x['allocated'] for x in candidates),'free_bytes_before':shutil.disk_usage(R).free,'preserved':'all current shared artifacts, all old and new acceptance executables, source/WIP and evidence, Cargo.lock and pristine package snapshots; future validation uses current all-target graph'}
(D/'cache-cleanup-before.json').write_text(json.dumps(record,indent=2)+'\n')
again=subprocess.run(['lsof','-nP','+D',str(R/'target')],text=True,capture_output=True)
assert again.returncode==1 and not again.stdout.strip() and not again.stderr.strip()
for p in selected:p.unlink()
record['free_bytes_after']=shutil.disk_usage(R).free;record['filesystem_available_delta_bytes']=record['free_bytes_after']-record['free_bytes_before'];record['all_selected_paths_absent']=all(not p.exists() for p in selected)
(D/'cache-cleanup.json').write_text(json.dumps(record,indent=2)+'\n');print(json.dumps({k:v for k,v in record.items() if k!='deleted'},indent=2))
