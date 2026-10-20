#!/usr/bin/env python3
"""Concurrent real Rust business and full-record L1 diagnostic, no Python simulation."""
import hashlib,json,os,subprocess,sys,time
from pathlib import Path
D=Path(__file__).resolve().parent;R=D.parents[2];EXE=R/'target/release/mycity-headless'
def write(p,v):
 with p.open('x') as f:json.dump(v,f,indent=2);f.write('\n');f.flush();os.fsync(f.fileno())
def main():
 root=Path(sys.argv[1]);root.mkdir();begin=time.monotonic();binary=hashlib.sha256(EXE.read_bytes()).hexdigest()
 write(root/'run-plan.json',{'L1_seconds':600,'scan_interval_seconds':1,'snapshot_interval_seconds':300,'business_launch_delay_after_L1_ready_seconds':120,'external_reservation_bytes':64*1024**2,'scope':'concurrent integration only; full-budget pressure not proven'})
 write(root/'manifest.json',{'external_owned_charge_bytes':6*1024**2,'scope':'Rust CPU diagnostics reservation; runner temporaries not fully accounted; joint budget not accepted'})
 handles=[];children=[]
 def start(name,args):
  out=(root/(name+'.stdout')).open('xb');error=(root/(name+'.stderr')).open('xb');handles.extend([out,error]);p=subprocess.Popen([str(EXE),*map(str,args)],stdout=out,stderr=error);children.append(p);return p
 layout=start('layout',['layout-charged-probe',root/'layout',600,300,64*1024**2])
 try:
  limit=time.monotonic()+60
  while not (root/'layout/manifest.json').exists():
   if layout.poll() is not None:raise RuntimeError('L1 died before ready')
   if time.monotonic()>limit:raise RuntimeError('L1 initialization exceeded diagnostic 60s')
   time.sleep(.05)
  ready=time.monotonic();write(root/'L1-ready.json',{'elapsed_seconds':ready-begin,'manifest':json.loads((root/'layout/manifest.json').read_text())})
  while time.monotonic()<ready+120:
   if layout.poll() is not None:raise RuntimeError('L1 terminated before business launch')
   time.sleep(.1)
  business=start('business',['scale-sim-probe',root/'business','full','measure']);launched=time.monotonic();business_done=None;layout_done=None
  while business_done is None or layout_done is None:
   for p in children:
    if p.poll() not in [None,0]:raise RuntimeError(f'child failed {p.pid}: {p.returncode}')
   if business_done is None and business.poll() is not None:business_done=time.monotonic()
   if layout_done is None and layout.poll() is not None:layout_done=time.monotonic()
   time.sleep(.1)
  assert hashlib.sha256(EXE.read_bytes()).hexdigest()==binary
  l=json.loads((root/'layout/report.json').read_text());b=json.loads((root/'business/report.json').read_text())
  assert l['all_published_scans_completed'] and l['scans']==600 and len(l['snapshots'])==2
  assert b['completed_hour_targets']==720 and b['verified_drive_targets']==723 and b['actual_worker_windows']==724
  latest=sorted((root/'layout/snapshots').glob('snapshot-*'))[-1]
  restore=start('restore',['layout-load-only',latest,root/'layout-restored'])
  if restore.wait()!=0:raise RuntimeError('actual full L1 restore failed')
  report={'schema':'mycity-joint-integration-v1','binary_sha256':binary,'L1_ready_elapsed_seconds':ready-begin,'business_launched_elapsed_seconds':launched-begin,'business_completed_elapsed_seconds':business_done-begin,'L1_completed_elapsed_seconds':layout_done-begin,'whole_seconds':time.monotonic()-begin,'latest_L1_snapshot':str(latest),'L1_cold_restore':'completed','full_business':'completed','joint_budget':'not_accepted_external_owned_and_unused_reservation_unproven','steady_gate':'not_run','eight_hour_gate':'not_run','memory_16gb_gate':'not_run','full_M0':'not_accepted'}
  write(root/'report.json',report);print(json.dumps(report,indent=2))
 finally:
  for p in children:
   if p.poll() is None:p.terminate()
  for p in children:
   if p.poll() is None:
    try:p.wait(timeout=10)
    except subprocess.TimeoutExpired:p.kill();p.wait()
  for f in handles:f.close()
if __name__=='__main__':main()
