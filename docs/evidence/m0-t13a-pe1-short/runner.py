#!/usr/bin/env python3
"""Fixed repetitions of original Rust P-E1 cases; Python never simulates economy."""
import hashlib,json,os,shutil,subprocess,sys,time,zipfile
from pathlib import Path
D=Path(__file__).resolve().parent;R=D.parents[2];EXE=R/'target/release/mycity-headless'
COUNTS={'debt':50,'prepay':8,'refund':8,'suspend':5}
KEYS={'debt':'debt_exit','prepay':'prepaid_cancel','refund':'refund_shortfall','suspend':'suspended_shift'}
def sha(p):
 h=hashlib.sha256()
 with p.open('rb') as f:
  while b:=f.read(65536):h.update(b)
 return h.hexdigest()
def sync(p):
 fd=os.open(p,os.O_RDONLY)
 try:os.fsync(fd)
 finally:os.close(fd)
def write(p,v):
 with p.open('x') as f:json.dump(v,f,indent=2);f.write('\n');f.flush();os.fsync(f.fileno())
 sync(p.parent)
def call(args,out):
 with out.open('xb') as f,out.with_suffix('.stderr').open('xb') as e:
  p=subprocess.run([str(EXE),*map(str,args)],stdout=f,stderr=e,env={**os.environ,'MYCITY_DIAGNOSTICS':'1'})
 if p.returncode:raise RuntimeError(f'CLI failed {args}: {p.returncode}')
 return json.loads(out.read_bytes())
def main():
 root=Path(sys.argv[1]);mode=sys.argv[2];assert mode in ['pilot','full'];root.mkdir();sync(root.parent)
 (root/'archives').mkdir();sync(root)
 counts=COUNTS if mode=='full' else dict.fromkeys(COUNTS,1)
 write(root/'run-plan.json',{'counts':counts,'mode':mode,'selection':'all fixed numbered trials','raw_cap':128*1024**2,'log_cap':256*1024**2,'scope':'P-E1 short-case sampling only'})
 charge={'external_owned_charge_bytes':6*1024**2,'scope':'conservative Rust diagnostics reservation; Python temporary owned memory incomplete; no joint budget acceptance'}
 write(root/'manifest.json',charge)
 if len(sys.argv)>3:
  observer_model=Path(sys.argv[3])/'model';observer_model.mkdir();write(observer_model/'manifest.json',charge)
 before=sha(EXE);reference=json.loads((D/'reference-worlds.json').read_text());rows=[];start=time.monotonic()
 for kind,n in counts.items():
  for i in range(n):
   ident=f'{kind}-{i:03}';trial=root/ident;trial.mkdir();sync(root)
   r=call(['economy-short-probe',R/'fixtures/m0-economy-opening.json',trial/'world',kind],trial/'cli.stdout')
   assert r['report']['world']==reference[KEYS[kind]],'complete original World mismatch'
   assert before==sha(EXE),'Rust executable changed'
   archive=root/'archives'/(ident+'.zip');files=[];sizes={'raw':0,'log':0,'model':0}
   with zipfile.ZipFile(archive,'x',zipfile.ZIP_DEFLATED) as z:
    for p in sorted(trial.rglob('*')):
     assert not p.is_symlink()
     if not p.is_file():continue
     name=p.relative_to(trial).as_posix();size=p.stat().st_size
     category='raw' if name.endswith('compute-durations.bin') else 'log' if 'diagnostics/' in name or not name.startswith('world/') else 'model'
     sizes[category]+=size;files.append({'path':name,'bytes':size,'sha256':sha(p)});z.write(p,name)
    z.writestr('archive-manifest.json',json.dumps(files))
   with archive.open('rb') as f:os.fsync(f.fileno())
   sync(archive.parent)
   restored=root/('restored-'+ident);restored.mkdir()
   with zipfile.ZipFile(archive) as z:
    assert set(z.namelist())=={v['path'] for v in files}|{'archive-manifest.json'}
    for row in files:
     h=hashlib.sha256();size=0
     with z.open(row['path']) as f:
      while b:=f.read(65536):h.update(b);size+=len(b)
     assert size==row['bytes'] and h.hexdigest()==row['sha256']
     if row['path']=='world/LOCK':continue
     dest=restored/row['path'];assert dest.resolve().is_relative_to(restored.resolve());dest.parent.mkdir(parents=True,exist_ok=True)
     with z.open(row['path']) as source,dest.open('xb') as out:shutil.copyfileobj(source,out,65536)
   proof=call(['economy-short-restore',restored/'world'],root/(ident+'-cold.stdout'))
   for key in ['world','state_sha256','committed_sequence']:assert proof[key]==r['report'][key]
   assert sizes['raw']<=128*1024 and sizes['log']<=256*1024
   rows.append({'trial':ident,'case':kind,'archive_sha256':sha(archive),'sizes':sizes,'active_transactions':r['active_transactions'],'successful_transactions':r['successful_transactions'],'source_sha256':r['source_sha256'],'cold_equal':True})
   write(root/(ident+'-receipt.json'),rows[-1])
   shutil.rmtree(trial);shutil.rmtree(restored)
 assert before==sha(EXE)
 result={'mode':mode,'trials':rows,'actual_wall_seconds':time.monotonic()-start,'binary_sha256':before,'binary_checked_before_and_after_each_trial':True,'formal_steady_gate':'not_run','eight_hour_gate':'not_run','memory_16gb_gate':'not_run','full_M0':'not_accepted'}
 write(root/'report.json',result);print(json.dumps({'completed':len(rows),'seconds':result['actual_wall_seconds']}))
if __name__=='__main__':main()
