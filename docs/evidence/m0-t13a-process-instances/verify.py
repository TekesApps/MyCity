#!/usr/bin/env python3
"""Independent terminal audit of process instances, raw roles and original full world."""
import collections,hashlib,json,re,sys
from pathlib import Path
R=Path(sys.argv[1]);M=R/'model';load=lambda p:json.loads(p.read_text());sha=lambda b:hashlib.sha256(b).hexdigest()
observer=load(R/'observer-report.json');report=load(M/'report.json')
assert observer['status']=='passed_measurement_wrapper' and observer['binary_unchanged'] and observer['observer_source_unchanged']
assert report['completed_hour_targets']==720 and report['verified_drive_targets']==723 and report['actual_worker_windows']==724
assert report['saved_sequence']==3011 and report['complete_final_business_and_air']=='passed' and report['cold_complete_image_unchanged']
assert report['oldest_receipt_retry']=='passed_without_side_effect' and report['attempt_failures']==0
heap={};roles=collections.Counter();peaks=collections.defaultdict(int)
for line in (M/'heaps.jsonl').read_text().splitlines():
 v=json.loads(line);key=(v['pid'],v['instance_unix_ns']);assert key not in heap and v['success']
 assert v['source_sha256']==report['source_sha256'] and v['tail_reserved_bytes']==65536
 assert v['heap']['lifetime_peak_requested_bytes']>=v['heap']['current_requested_bytes']
 heap[key]=v;roles[v['role']]+=1;peaks[v['role']]=max(peaks[v['role']],v['heap']['lifetime_peak_requested_bytes'])
assert roles=={'scale-sim-probe':1,'sim-host-scale-full':2,'compute-worker-bounded':724},roles
producer_dirs=set();producer_keys=set();process_keys=set();producer_counts=collections.Counter();raw_count=0;raw_bytes=0;charges=collections.defaultdict(int)
classes=['L0_pe0_model_transaction','L0_pe1_model_transaction','L2_claims_batch','L2_payroll_batch','L2_region_exchange_boot_batch','L2_background_capture_admit_cpu','air_source_page','air_cell_page','L1_native_record_index_page','L1_stub_record_index_page']
for p in (M/'diagnostics').glob('*/report.json'):
 producer_dirs.add(p.parent)
 d=load(p);kind=d['producer'];assert kind in ('sim','worker')
 identity=(d['process_pid'],d['process_instance_unix_ns']);key=identity+(d['producer_instance'],)
 assert key not in producer_keys;producer_keys.add(key);process_keys.add(identity)
 assert d['producer_directory']==p.parent.name==f"{kind}-{identity[0]}-{d['producer_instance']}"
 assert identity in heap and heap[identity]['role']==('compute-worker-bounded' if kind=='worker' else 'sim-host-scale-full')
 assert d['whole_process_rust_heap']['lifetime_peak_requested_bytes']<=heap[identity]['heap']['lifetime_peak_requested_bytes']
 assert d['charged_diagnostics_bound_bytes']==d['owned_diagnostics_bytes']+256*1024<=(1 if kind=='worker' else 3)*1024**2
 charges[kind]=max(charges[kind],d['charged_diagnostics_bound_bytes']);producer_counts[kind]+=1
 for i,role in enumerate(['execution','quote','replay']):
  counts=collections.Counter();maxima=collections.defaultdict(int);path=p.parent/role/'compute-durations.bin'
  with path.open('rb') as f:
   while chunk:=f.read(9*8192):
    assert len(chunk)%9==0;raw_bytes+=len(chunk)
    for offset in range(0,len(chunk),9):
     k=chunk[offset];assert k<10;n=int.from_bytes(chunk[offset+1:offset+9],'little');counts[classes[k]]+=1;maxima[classes[k]]=max(maxima[classes[k]],n);raw_count+=1
  for k in classes:
   row=d['role_compute'][i]['classes'][k];assert row['samples']==counts[k]
   if counts[k]:assert row['maximum_ns']==maxima[k]
assert producer_dirs=={p for p in (M/'diagnostics').iterdir() if p.is_dir()},'unsealed producer directory'
assert producer_counts=={'sim':2,'worker':724}
assert process_keys=={key for key,v in heap.items() if v['role']!='scale-sim-probe'}
# Same PID may legitimately appear repeatedly. Never collapse it to a dict keyed by PID.
assert len(producer_keys)==726
world=M/'world';c=load(world/'CURRENT.json');b=(world/f"root-{c['slot']}.json").read_bytes();assert sha(b)==c['root_sha256'];root=json.loads(b)
ref=root['checkpoint_v2'];b=(world/f"world-index-{ref['generation']}.json").read_bytes();assert sha(b)==ref['sha256'];index=json.loads(b);point=index['current']
b=(world/f"world-{point['generation']}.json").read_bytes();assert sha(b)==point['sha256'] and len(b)==point['bytes']
actual=json.loads(b);expected=load(M/'cpu-final-engine.json');assert actual==expected,'full Engine differs, including metadata'
ref=index['receipts'];last=root['sequence'];seen=set()
while ref:
 b=(world/f"receipt-index-{ref['generation']}.json").read_bytes();assert sha(b)==ref['sha256'];idx=json.loads(b)
 b=(world/f"receipts-{ref['generation']}.json").read_bytes();assert len(b)==idx['body_bytes'] and sha(b)==idx['body_sha256'];body=json.loads(b)
 assert body['last']==idx['last']==last and body['first']==idx['first'] and body['previous']==idx['previous']
 assert body['genesis_sha256']==idx['genesis_sha256']==root['genesis_sha256']
 assert len(body['receipts'])==idx['last']-idx['first']+1 and [r['operation_id'] for r in body['receipts']]==idx['operations']
 for receipt in body['receipts']:
  assert 'Ok' in receipt['outcome'] and receipt['operation_id'] not in seen;seen.add(receipt['operation_id'])
 last=idx['first']-1;ref=idx['previous']
assert last==0 and len(seen)==3011
frames=0
for line in (R/'host-samples.jsonl').read_text().splitlines():
 v=json.loads(line);assert v['rss_bytes']==sum(member[3] for member in v['members']);frames+=1
request=0;pending=False;responses=0;max_row=0
with (M/'transcript.jsonl').open('rb') as f:
 for line in f:
  max_row=max(max_row,len(line));assert len(line)<=128*1024+1
  d=json.loads(line)['data']
  if 'request' in d:
   assert not pending and d['request']==request+1;request+=1;pending=True
  elif 'response' in d:
   assert pending and d['response']==request and re.fullmatch('[a-f0-9]{64}',d['payload_sha256'])
   assert d['payload_bytes']<=65536;responses+=1
   if d['kind']=='result':pending=False
assert not pending
result={'schema':'mycity-t13a-instance-audit-v1','source_sha256':report['source_sha256'],'status':'full_30day_instance_and_copy_change_verified','process_records':dict(roles),'producer_process_pairs':len(process_keys),'producer_directories':len(producer_keys),'all_report_instances_match_heap_records':True,'all_role_raw_counts_and_maxima_match':True,'raw_samples':raw_count,'raw_bytes':raw_bytes,'original_receipts_verified':len(seen),'entire_engine_equal_cpu_including_metadata':True,'RSS_frames_checked':frames,'transcript_requests':request,'transcript_responses':responses,'maximum_transcript_row_bytes':max_row,'requested_Rust_heap_role_lifetime_peaks_bytes':dict(peaks),'diagnostic_charges_bytes':dict(charges),'diagnostic_total':'not_closed','full_joint_owned':'not_accepted','four_speeds':'not_run','formal_steady':'not_run','eight_hour':'not_run','real_16GB':'not_run'}
print(json.dumps(result,indent=2))
