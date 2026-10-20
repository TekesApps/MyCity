#!/usr/bin/env python3
"""Offline audit of complete business, fixed L1 record counts, overlap, files and RSS sums."""
import hashlib,json,sys,zipfile
from pathlib import Path
D=Path(__file__).resolve().parent;root=Path(sys.argv[1]);model=root/'model'
load=lambda p:json.loads(p.read_text())
sha=lambda b:hashlib.sha256(b).hexdigest()
def file_sha(p):
 h=hashlib.sha256()
 with p.open('rb') as f:
  while b:=f.read(65536):h.update(b)
 return h.hexdigest()
business=load(model/'business/report.json');layout=load(model/'layout/report.json');observer=load(root/'observer-report.json')
assert observer['status']=='model_failed' and observer['binary_unchanged'] and observer['observer_source_unchanged']
assert (model/'restore.stderr').read_text().strip()=='layout_restore_work_budget'
repaired=load(Path(sys.argv[2])/'model/report.json')
assert repaired['cold_reopen_and_scan']=='passed' and not repaired['next_generation_requested']
assert repaired['manifest']['mutation_iteration']==300 and repaired['resumed_scan']['iteration']==301
assert repaired['resumed_scan']['native_records']==361272 and repaired['resumed_scan']['stub_records']==1232426
assert business['completed_hour_targets']==720 and business['verified_drive_targets']==723 and business['actual_worker_windows']==724 and business['complete_final_business_and_air']=='passed' and business['cold_complete_image_unchanged']
assert layout['scans']==600 and layout['all_published_scans_completed'] and layout['snapshot_interval_seconds']==300
assert layout['manifest']['external_owned_charge_bytes']==64*1024**2 and layout['manifest']['normal_joint_budget_bytes']==310*1024**3//100
assert len(layout['snapshots'])==2
scans=[];admitted=[];durable=[]
for line in (model/'layout/events.jsonl').read_text().splitlines():
 e=json.loads(line)
 if 'scan' in e:
  assert e['scan']['native_records']==361272 and e['scan']['stub_records']==1232426;scans.append(e)
 if 'snapshot_admitted' in e:admitted.append(e)
 if 'snapshot_durable' in e:durable.append(e)
assert [e['scan_deadline_seconds'] for e in scans]==list(range(600))
assert [e['snapshot_deadline_seconds'] for e in admitted]==[0,300] and len(durable)==2
for e in durable:assert e['snapshot_durable']['complete_record_restore']=='passed' and e['snapshot_durable']['full_pressure_file_readback']=='passed'
second_begin=admitted[1]['elapsed_seconds'];second_end=durable[1]['elapsed_seconds']
transcript=[json.loads(l) for l in (model/'business/transcript.jsonl').read_text().splitlines()]
verified=[e for e in transcript if 'verified_target' in e['data']];assert len(verified)==723
witness=load(root/'overlap-witness.json')
for mark in [witness['first'],witness['last']]:
 assert mark['pending_snapshot_observed_before_and_after_read']
 current=json.loads(mark['current_bytes']);rb=mark['root_bytes'].encode()
 assert sha(rb)==current['root_sha256'] and json.loads(rb)['sequence']==mark['sequence']
assert witness['last']['sequence']>witness['first']['sequence']
assert witness['last']['observed_monotonic_seconds']>witness['first']['observed_monotonic_seconds']
with (model/'business/transcript.jsonl').open('rb') as f:
 lo,hi=witness['transcript_byte_range'];f.seek(lo);segment=f.read(hi-lo)
for row in witness['complete_targets_observed']:
 candidates=[line for line in segment.splitlines() if sha(line+b'\n')==row['sha256']]
 assert len(candidates)==1 and json.loads(candidates[0])==row['line']
assert witness['complete_targets_observed']
frames=[json.loads(l) for l in (root/'host-samples.jsonl').read_text().splitlines()]
for f in frames:assert f['rss_bytes']==sum(m[3] for m in f['members'])
# Snapshot full-byte hashes audited only after all writers have stopped.
checked_files=0;checked_bytes=0
for snap in sorted((model/'layout/snapshots').glob('snapshot-*')):
 index=load(snap/'index.json')
 for row in index['files']:
  p=snap/row['name'];assert p.stat().st_size==row['bytes'] and file_sha(p)==row['sha256'];checked_files+=1;checked_bytes+=row['bytes']
# CURRENT -> root -> world index -> complete actual Engine, independently checked.
world=model/'business/world';cur=load(world/'CURRENT.json');rb=(world/f"root-{cur['slot']}.json").read_bytes();assert sha(rb)==cur['root_sha256'];rt=json.loads(rb)
ref=rt['checkpoint_v2'];ib=(world/f"world-index-{ref['generation']}.json").read_bytes();assert sha(ib)==ref['sha256'];point=json.loads(ib)['current'];engine=(world/f"world-{point['generation']}.json").read_bytes();assert len(engine)==point['bytes'] and sha(engine)==point['sha256']
index=json.loads(ib);ref=index['receipts'];last=rt['sequence'];receipt_count=0;operations=set()
while ref is not None:
 b=(world/f"receipt-index-{ref['generation']}.json").read_bytes();assert sha(b)==ref['sha256'];idx=json.loads(b)
 b=(world/f"receipts-{ref['generation']}.json").read_bytes();assert len(b)==idx['body_bytes'] and sha(b)==idx['body_sha256'];body=json.loads(b)
 assert body['genesis_sha256']==idx['genesis_sha256']==rt['genesis_sha256']
 assert body['first']==idx['first'] and body['last']==idx['last']==last and body['previous']==idx['previous']
 assert idx['last']-idx['first']+1==len(body['receipts'])==len(idx['operations'])
 assert [v['operation_id'] for v in body['receipts']]==idx['operations']
 for v in body['receipts']:
  assert v['operation_id'] not in operations and 'Ok' in v['outcome'];operations.add(v['operation_id'])
 receipt_count+=len(body['receipts']);last=idx['first']-1;ref=idx['previous']
assert last==0 and receipt_count==3011==rt['sequence']
result={'status':'original_joint_run_failed_restore_guard; fixed_reader_same_snapshot_verified','snapshot_files_checked':checked_files,'snapshot_bytes_checked':checked_bytes,'L1_scan_count':len(scans),'native_samples_expected':47*600,'stub_samples_expected':168*600,'second_snapshot_L1_clock_interval':[second_begin,second_end],'business_receipt_chain_checked':receipt_count,'business_verified_targets':len(verified),'RSS_frames_checked':len(frames),'sampled_group_peak_rss_bytes':max(f['rss_bytes'] for f in frames),'actual_overlap_witness':{'durable_sequence_before':witness['first']['sequence'],'durable_sequence_after':witness['last']['sequence'],'complete_targets':len(witness['complete_targets_observed']),'outside_measured_tree':True},'pressure_budget':'not_accepted; 64MiB unused reservation not filled and complete external owned missing','memory_16gb_gate':'not_run','eight_hour_gate':'not_run'}
print(json.dumps(result,indent=2))
