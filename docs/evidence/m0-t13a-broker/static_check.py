#!/usr/bin/env python3
"""Source capsules and retained attempt provenance. Does not turn failures into passes."""
import hashlib,json,struct,zipfile
from pathlib import Path
D=Path(__file__).resolve().parent;R=D.parents[2];load=lambda p:json.loads(p.read_text());sha=lambda b:hashlib.sha256(b).hexdigest()
verified=[]
for folder,current in [(D/'attempt1-source',False),(D/'attempt2-source',False),(D,True)]:
 info=load(folder/'source.json');h=hashlib.sha256()
 with zipfile.ZipFile(folder/'source.zip') as z:
  assert len(z.namelist())==info['source_files']
  for n in sorted(z.namelist()):
   b=z.read(n);p=n.encode();h.update(struct.pack('<Q',len(p)));h.update(p);h.update(struct.pack('<Q',len(b)));h.update(b)
   if current:assert b==(R/n).read_bytes(),n
  assert sha(z.read('fixtures/m0-t13a-acceptance.json'))==info['frozen_acceptance_sha256']=='876ea995ea7834bfcea59b2ef8fa56d502aed9006477bcc67bcaa3f5cd944508'
 assert h.hexdigest()==info['source_sha256']
 assert sha((folder/'source-support.zip').read_bytes())==info['source_support_sha256']
 if current:assert sha((R/'target/release/mycity-headless').read_bytes())==info['binary_sha256']
 verified.append({'folder':folder.name,'source_sha256':info['source_sha256'],'files':info['source_files'],'matches_current_workspace':current})
info=load(D/'source.json');assert sha((D/'observe.py').read_bytes())==info['observer_sha256']
for name,source_folder in [('t13a3j-functional-01','attempt1-source'),('t13a3m-functional-01','.')]:
 p=D/name
 if not p.exists():continue
 rows=load(p/'manifest.json');source=load(D/source_folder/'source.json')
 with zipfile.ZipFile(p/'artifacts.zip') as z:
  assert set(z.namelist())=={r['path'] for r in rows}
  for r in rows:
   b=z.read(r['path']);assert len(b)==r['bytes'] and sha(b)==r['sha256']
  o=json.loads(z.read('observer-report.json'));m=json.loads(z.read('model/manifest.json'))
  assert m['source_sha256']==source['source_sha256'] and m['rust_binary_sha256']==source['binary_sha256']
  assert o['binary_unchanged'] and o['observer_source_unchanged'] and o['observer_source_sha256']==source['observer_sha256']
  if o['status']=='passed_measurement_wrapper':
   b=json.loads(z.read('model/business/report.json'));pool=json.loads(z.read('model/normal-pool/report.json'))
   assert b['completed_hour_targets']==720 and b['verified_drive_targets']==723 and b['actual_worker_windows']==724 and b['saved_sequence']==3011
   assert pool['balanced_after_owned_wait'] and pool['summary']['grants']==pool['summary']['returns']==pool['summary']['fill_acknowledgements']==724
  verified.append({'attempt':name,'status':o['status'],'source_sha256':source['source_sha256'],'archive_files':len(rows)})
print(json.dumps({'verified':verified,'scope':'source and archive integrity only; separate runtime verifiers required','L1_joint':'not_accepted','diagnostic_total':'not_closed','formal_steady':'not_run','four_speeds':'not_run','eight_hour':'not_run','real_16GB':'not_run','full_M0':'not_accepted'},indent=2))
