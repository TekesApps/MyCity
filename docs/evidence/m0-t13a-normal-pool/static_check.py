#!/usr/bin/env python3
"""Frozen source/binary/observer and terminal archive integrity. Does not infer pass."""
import hashlib,json,struct,zipfile
from pathlib import Path
D=Path(__file__).resolve().parent;R=D.parents[2];sha=lambda b:hashlib.sha256(b).hexdigest();load=lambda p:json.loads(p.read_text());info=load(D/'source.json');h=hashlib.sha256()
with zipfile.ZipFile(D/'source.zip') as z:
 assert len(z.namelist())==info['source_files']
 for name in sorted(z.namelist()):
  b=z.read(name);assert b==(R/name).read_bytes(),name;n=name.encode();h.update(struct.pack('<Q',len(n)));h.update(n);h.update(struct.pack('<Q',len(b)));h.update(b)
 assert sha(z.read('fixtures/m0-t13a-acceptance.json'))==info['frozen_acceptance_sha256']=='876ea995ea7834bfcea59b2ef8fa56d502aed9006477bcc67bcaa3f5cd944508'
assert h.hexdigest()==info['source_sha256']
assert sha((R/'target/release/mycity-headless').read_bytes())==info['binary_sha256']
assert sha((D/'observe.py').read_bytes())==info['observer_sha256']
assert sha((D/'source-support.zip').read_bytes())==info['source_support_sha256']
with zipfile.ZipFile(D/'source-support.zip') as z:
 for name in z.namelist():assert z.read(name)==(R/name).read_bytes()
old_info=load(D/'attempt2-source/source.json')
p=D/'t13a3h-joint-01';rows=load(p/'manifest.json')
with zipfile.ZipFile(p/'artifacts.zip') as z:
 assert set(z.namelist())=={r['path'] for r in rows}
 for row in rows:
  b=z.read(row['path']);assert len(b)==row['bytes'] and sha(b)==row['sha256']
 o=json.loads(z.read('observer-report.json'));assert o['binary_sha256']==old_info['binary_sha256'] and o['observer_source_sha256']==info['observer_sha256'] and o['binary_unchanged'] and o['observer_source_unchanged']
 frame_count=0
 for line in z.read('host-samples.jsonl').splitlines():
  v=json.loads(line);assert v['rss_bytes']==sum(m[3] for m in v['members']);frame_count+=1
 assert frame_count==o['samples']
 if o['status']=='passed_measurement_wrapper':
  q=json.loads(z.read('model/report.json'));b=json.loads(z.read('model/business/report.json'));pool=[json.loads(line) for line in z.read('model/normal-pool/events.jsonl').splitlines()][-1]
  assert q['source_sha256']==b['source_sha256']==pool['source_sha256']==info['source_sha256']
  assert b['completed_hour_targets']==720 and b['actual_worker_windows']==724 and b['saved_sequence']==3011 and pool['balanced_after_owned_wait']
  assert pool['summary']['grants']==pool['summary']['returns']==pool['summary']['fill_acknowledgements']==724
 elif o['status']=='failed':
  assert o['reason'] and not (D/'joint-verification.json').exists(),'a failed attempt cannot be signed by success verification'
 else:raise AssertionError(o['status'])
r=load(D/'readiness.json')['samples'];assert len(r)==25 and all(s['pressure']==1 for s in r)
span=r[-1]['elapsed_seconds']-r[0]['elapsed_seconds'];gap=max(b['elapsed_seconds']-a['elapsed_seconds'] for a,b in zip(r,r[1:]));assert span>=120 and gap<6
print(json.dumps({'source_sha256':info['source_sha256'],'binary_sha256':info['binary_sha256'],'source_files_verified':info['source_files'],'archived_files_verified':len(rows),'observer_terminal_status':o['status'],'RSS_frames_verified':frame_count,'normal_readiness_span_seconds':span,'readiness_max_gap_seconds':gap,'large_L1_payload':'retained_in_ignored_work_with_original_snapshot_index_hashes','exact_normal_owned':'not_accepted','formal_steady':'not_run','four_speeds':'not_run','eight_hour':'not_run','real_16GB':'not_run'},indent=2))

# Preserve the failed full-business run independently of the corrected candidate.
p=D/'t13a3h-functional-01';rows=load(p/'manifest.json')
with zipfile.ZipFile(p/'artifacts.zip') as z:
 assert set(z.namelist())=={r['path'] for r in rows}
 for row in rows:
  b=z.read(row['path']);assert len(b)==row['bytes'] and sha(b)==row['sha256']
 o=json.loads(z.read('observer-report.json'));v=json.loads(z.read('model/runner-report.json'));m=json.loads(z.read('model/manifest.json'));pool=[json.loads(line) for line in z.read('model/normal-pool/events.jsonl').splitlines()][-1]
 assert o['status']=='model_failed' and v['status']=='failed'
 assert m['source_sha256']==old_info['source_sha256'] and m['rust_binary_sha256']==old_info['binary_sha256']
 assert pool['summary']['grants']==pool['summary']['returns']==125 and pool['summary']['fill_acknowledgements']==124 and pool['summary']['rejected_requests']==2
 assert b'normal_pool_request_identity' in z.read('model/business.stderr')
 assert pool['owner_actual_capacity_bytes']==pool['summary']['available_touched_bytes']==67108864
 assert 'model/normal-pool/report.json' not in z.namelist(), 'failed launcher terminated pool; no normal exit proof'
print(json.dumps({'failed_functional_archive_verified':len(rows),'status':'model_failed_preserved','grants':125,'returns':125,'filled':124,'rejected':2}))

# Corrected-source complete business, archived independently of both failed attempts.
p=D/'t13a3i-functional-01';rows=load(p/'manifest.json')
with zipfile.ZipFile(p/'artifacts.zip') as z:
 assert set(z.namelist())=={r['path'] for r in rows}
 for row in rows:
  b=z.read(row['path']);assert len(b)==row['bytes'] and sha(b)==row['sha256']
 o=json.loads(z.read('observer-report.json'));m=json.loads(z.read('model/manifest.json'));v=json.loads(z.read('model/runner-report.json'));pool=json.loads(z.read('model/normal-pool/report.json'));business=json.loads(z.read('model/business/report.json'))
 assert o['status']=='passed_measurement_wrapper' and v['status']=='completed_functional_only'
 assert o['binary_sha256']==m['runner_sha256']==v['runner_sha256']==sha((D/'pool_full.py').read_bytes())
 assert m['rust_binary_sha256']==v['rust_binary_sha256']==info['binary_sha256']
 assert m['source_sha256']==v['source_sha256']==pool['source_sha256']==business['source_sha256']==info['source_sha256']
 assert business['completed_hour_targets']==720 and business['verified_drive_targets']==723 and business['actual_worker_windows']==724 and business['saved_sequence']==3011
 assert pool['balanced_after_owned_wait'] and pool['summary']['grants']==pool['summary']['returns']==pool['summary']['fill_acknowledgements']==724 and pool['summary']['rejected_requests']==0
 assert pool['summary']['available_touched_bytes']==67108864 and pool['summary']['granted_capacity_bytes']==0
 assert len(z.read('model/heaps.jsonl').splitlines())==731
ready=load(D/'functional-readiness-after-fix.json')['samples'];assert len(ready)==25 and all(s['pressure']==1 for s in ready)
assert ready[-1]['elapsed_seconds']-ready[0]['elapsed_seconds']>=120
print(json.dumps({'corrected_functional_archive_files_verified':len(rows),'source_sha256':info['source_sha256'],'status':'completed_functional_only','joint':'prior_failure_preserved_not_accepted','four_speeds':'not_run','eight_hour':'not_run','real_16GB':'not_run'}))
