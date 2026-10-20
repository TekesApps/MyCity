#!/usr/bin/env python3
"""Sequential actual Rust CLI trials. Copies/archives only; no Python city simulation."""
import hashlib,json,os,shutil,subprocess,sys,time,zipfile
from pathlib import Path
D=Path(__file__).resolve().parent;R=D.parents[2];EXE=R/'target/release/mycity-headless'
def sync_dir(p):
 fd=os.open(p,os.O_RDONLY)
 try:os.fsync(fd)
 finally:os.close(fd)
def write(p,v):
 b=(json.dumps(v,indent=2)+'\n').encode();tmp=p.with_suffix('.pending')
 with tmp.open('wb') as f:f.write(b);f.flush();os.fsync(f.fileno())
 tmp.replace(p);sync_dir(p.parent)
def sha_file(p):
 h=hashlib.sha256()
 with p.open('rb') as f:
  while b:=f.read(65536):h.update(b)
 return h.hexdigest()
def call(args,out):
 with out.open('wb') as stdout,out.with_suffix('.stderr').open('wb') as stderr:
  p=subprocess.run([str(EXE)]+[str(a) for a in args],stdout=stdout,stderr=stderr)
 if p.returncode:raise RuntimeError('CLI failed '+str(args)+' exit='+str(p.returncode))
 return json.loads(out.read_bytes())
def classify(name):
 if name.endswith('/compute-durations.bin'):return 'raw'
 if not name.startswith('world/'):return 'log'
 return 'model'
def archive(trial,archive,expected,restore_dir):
 files=[];sizes={'raw':0,'log':0,'model':0}
 with zipfile.ZipFile(archive,'x',zipfile.ZIP_DEFLATED) as z:
  for p in sorted(trial.rglob('*')):
   if p.is_symlink():raise RuntimeError('trial symlink')
   if not p.is_file():continue
   name=p.relative_to(trial).as_posix();size=p.stat().st_size
   files.append({'path':name,'bytes':size,'sha256':sha_file(p)})
   sizes[classify(name)]+=size;z.write(p,name)
  z.writestr('archive-manifest.json',json.dumps(files,indent=2)+'\n')
 with archive.open('rb') as f:os.fsync(f.fileno())
 sync_dir(archive.parent)
 # Read every archived byte and check the exact pre-archive manifest before restoration.
 with zipfile.ZipFile(archive) as z:
  assert set(z.namelist())=={v['path'] for v in files}|{'archive-manifest.json'}
  for row in files:
   h=hashlib.sha256();n=0
   with z.open(row['path']) as f:
    while b:=f.read(65536):h.update(b);n+=len(b)
   assert n==row['bytes'] and h.hexdigest()==row['sha256']
  restore_dir.mkdir()
  runtime=['world/LOCK','world/air-worker.lock','world/air-worker-owner.json']
  for name in z.namelist():
   if name in runtime or name.startswith('world/air-worker/'):continue
   p=restore_dir/name
   assert p.resolve().is_relative_to(restore_dir.resolve())
   p.parent.mkdir(parents=True,exist_ok=True)
   with z.open(name) as source,p.open('xb') as dest:shutil.copyfileobj(source,dest,65536)
 proof=call(['scale-rare-restore',restore_dir/'world',expected],restore_dir/'cold-proof.stdout')
 assert proof['cold_archive_restore']=='passed'
 return sizes,proof,len(files)
def main():
 root=Path(sys.argv[1]);mode=sys.argv[2];assert mode in ['pilot','full'];root.mkdir();sync_dir(root.parent)
 counts={'claims':200,'source':100} if mode=='full' else {'claims':1,'source':1}
 plan={'schema':'mycity-rare-repetitions-v1','mode':mode,'strata':counts,'raw_allowance_per_trial':128*1024,'log_allowance_per_trial':256*1024,'selection':'fixed all numbered trials; no latency selection','raw_global_cap':128*1024**2,'logs_global_cap':256*1024**2,'scope':'actual restored full-world Sim branches; not continuous world/steady/8h'}
 write(root/'run-plan.json',plan)
 write(root/'manifest.json',{'external_owned_charge_bytes':6*1024**2,'scope':'one Sim + one worker reserved; Python runner temp/accounting remains explicit, no joint memory acceptance'})
 anchors=root/'anchors';start=time.monotonic();call(['scale-rare-anchors',anchors],root/'anchors.stdout')
 archives=root/'archives';archives.mkdir();sync_dir(root);done=0;raw=0;logs=0;by_kind={};summary={'source_sha256':json.loads((anchors/'report.json').read_text())['source_sha256'],'completed':0}
 with (root/'trials.jsonl').open('xb') as ledger:
  sync_dir(root)
  for kind,count in counts.items():
   for i in range(count):
    ident=f'{kind}-{i:03d}';trial=root/ident;at=time.monotonic()
    reserved_raw=(done+1)*plan['raw_allowance_per_trial'];reserved_logs=(done+1)*plan['log_allowance_per_trial']
    if reserved_raw>plan['raw_global_cap'] or reserved_logs+64*1024**2>plan['logs_global_cap']:raise RuntimeError('global diagnostics admission')
    r=call(['scale-rare-trial',anchors,trial,kind],root/(ident+'.stdout'))
    # Command output belongs to this trial's bounded diagnostic archive.
    (root/(ident+'.stdout')).replace(trial/'cli.stdout');(root/(ident+'.stderr')).replace(trial/'cli.stderr')
    assert r['complete_engine_equal'] and r['cold_reopen_equal']
    samples={'claims':0,'sim_source':0,'worker_source':0}
    for p in trial.glob('diagnostics/*/report.json'):
     d=json.loads(p.read_text());c=d['role_compute'][0]['classes'];samples['claims']+=c['L2_claims_batch']['samples']
     if d['producer']=='sim':samples['sim_source']+=c['air_source_page']['samples']
     elif d['producer']=='worker':samples['worker_source']+=c['air_source_page']['samples']
    assert samples==({'claims':1,'sim_source':0,'worker_source':0} if kind=='claims' else {'claims':0,'sim_source':2,'worker_source':2})
    archive_path=archives/(ident+'.zip');restore_dir=root/('restored-'+ident)
    sizes,proof,files=archive(trial,archive_path,anchors/kind/'expected-engine.json',restore_dir)
    if sizes['raw']>plan['raw_allowance_per_trial'] or sizes['log']>plan['log_allowance_per_trial']:raise RuntimeError('trial diagnostic reservation exceeded')
    raw+=sizes['raw'];logs+=sizes['log'];done+=1;by_kind[kind]=by_kind.get(kind,0)+1
    row={'trial':ident,'stratum':kind,'samples':samples,'report':r,'archived_files':files,'archive_bytes':archive_path.stat().st_size,'archive_sha256':sha_file(archive_path),'uncompressed_sizes':sizes,'restore_proof':proof,'whole_trial_wall_seconds_including_archive':time.monotonic()-at}
    ledger.write((json.dumps(row,separators=(',',':'))+'\n').encode());ledger.flush();os.fsync(ledger.fileno())
    # Only owned, cold-verified, durably archived diagnostic copies are reclaimed.
    shutil.rmtree(trial);shutil.rmtree(restore_dir)
    summary={'schema':'mycity-rare-repetition-result-v1','source_sha256':summary['source_sha256'],'mode':mode,'completed':done,'by_stratum':by_kind,'actual_raw_bytes_archived':raw,'actual_log_bytes_archived':logs,'reserved_raw_bytes':done*plan['raw_allowance_per_trial'],'reserved_log_bytes':done*plan['log_allowance_per_trial'],'all_archives_verified_and_cold_reopened':True,'actual_wall_seconds':time.monotonic()-start,'formal_10class_gate':'not_accepted_here','steady_three_runs':'not_run','eight_hour_gate':'not_run','memory_16gb_gate':'not_run','full_M0':'not_accepted'}
    write(root/'report.json',summary)
    if done%20==0:print(json.dumps({'completed':done,'stratum':kind,'raw_bytes':raw,'log_bytes':logs}),flush=True)
 print(json.dumps(summary,indent=2))
if __name__=='__main__':main()
