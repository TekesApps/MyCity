#!/usr/bin/env python3
"""Independent fixed-load, per-process heap completeness and diagnostic ownership audit."""
import array,collections,hashlib,json,math,sys,zipfile
from pathlib import Path
R=Path(sys.argv[1]);M=R/'model';load=lambda p:json.loads(p.read_text());sha=lambda b:hashlib.sha256(b).hexdigest();MiB=1024**2
observer=load(R/'observer-report.json');report=load(M/'report.json');business=load(M/'business/report.json');layout=load(M/'layout/report.json');restored=load(M/'layout-restored/report.json')
assert observer['status']=='passed_measurement_wrapper' and observer['binary_unchanged'] and observer['observer_source_unchanged']
assert business['completed_hour_targets']==720 and business['verified_drive_targets']==723 and business['actual_worker_windows']==724 and business['saved_sequence']==3011
assert business['complete_final_business_and_air']=='passed' and business['cold_complete_image_unchanged'] and business['oldest_receipt_retry']=='passed_without_side_effect'
assert layout['scans']==600 and layout['all_published_scans_completed'] and layout['snapshot_summaries_only'] and len(layout['snapshots'])==2
assert layout['diagnostics']['charged_bound_bytes']<=MiB
assert restored['cold_reopen_and_scan']=='passed' and restored['restore']['same_source'] and restored['resumed_scan']['iteration']==301
old=Path(__file__).resolve().parents[1]/'m0-t13a-joint/t13a3a-joint-01/artifacts.zip'
with zipfile.ZipFile(old) as z:
 for generation in [1,2]:
  name=f'model/layout/snapshots/snapshot-{generation:06}/index.json'
  assert json.loads(z.read(name))['files']==load(R/name)['files'],'physical L1 input payload changed'
counts=collections.Counter();heap={};peaks=collections.defaultdict(int)
for line in (M/'heaps.jsonl').read_text().splitlines():
 v=json.loads(line);identity=(v['pid'],v['instance_unix_ns']);assert identity not in heap and v['success']
 assert v['source_sha256']==report['source_sha256'] and v['heap']['lifetime_peak_requested_bytes']>=v['heap']['current_requested_bytes'] and v['tail_reserved_bytes']==65536
 heap[identity]=v;counts[v['role']]+=1;peaks[v['role']]=max(peaks[v['role']],v['heap']['lifetime_peak_requested_bytes'])
assert counts=={'joint-budget-probe':1,'layout-charged-probe':1,'scale-sim-probe':1,'sim-host-scale-full':2,'compute-worker-bounded':724,'layout-load-only':1},counts
by_pid=collections.defaultdict(list)
for v in heap.values():by_pid[v['pid']].append(v)
charged=collections.defaultdict(int);producers=collections.Counter();rawbytes=0;hists={};maxima=collections.defaultdict(int);samples=collections.Counter()
classes=['L0_pe0_model_transaction','L0_pe1_model_transaction','L2_claims_batch','L2_payroll_batch','L2_region_exchange_boot_batch','L2_background_capture_admit_cpu','air_source_page','air_cell_page','L1_native_record_index_page','L1_stub_record_index_page']
def raw(path,group,expected=None):
 global rawbytes
 observed=collections.Counter()
 with path.open('rb') as f:
  while b:=f.read(9):
   assert len(b)==9 and b[0]<10;class_id=b[0];ns=int.from_bytes(b[1:],'little');key=(group,classes[class_id]);bucket=min(40001,(ns+999)//1000)
   if key not in hists:hists[key]=array.array('I',[0])*40002
   hists[key][bucket]+=1;samples[key]+=1;maxima[key]=max(maxima[key],ns);observed[classes[class_id]]+=1;rawbytes+=9
 if expected is not None:
  assert all(expected[k]['samples']==observed[k] for k in classes)
for p in (M/'business/diagnostics').glob('*/report.json'):
 d=load(p);kind=d['producer'];assert kind in ['sim','worker'];cap=(3 if kind=='sim' else 1)*MiB
 assert d['charged_diagnostics_bound_bytes']==d['owned_diagnostics_bytes']+256*1024<=cap
 charged[kind]=max(charged[kind],d['charged_diagnostics_bound_bytes']);producers[kind]+=1
 pid=int(p.parent.name.split('-')[1]);matches=[v for v in by_pid[pid] if v['role'].startswith('sim-host') or v['role']=='compute-worker-bounded'];assert len(matches)==1
 assert matches[0]['heap']['lifetime_peak_requested_bytes']>=d['whole_process_rust_heap']['lifetime_peak_requested_bytes']
 for i,role in enumerate(['execution','quote','replay']):raw(p.parent/role/'compute-durations.bin',kind+'/'+role,d['role_compute'][i]['classes'])
assert producers=={'sim':2,'worker':724}
raw(M/'layout/compute-durations.bin','layout/execution',layout['compute']['classes'])
assert samples[('layout/execution','L1_native_record_index_page')]==600*47 and samples[('layout/execution','L1_stub_record_index_page')]==600*168
scan=0;snap=[]
for line in (M/'layout/events.jsonl').read_text().splitlines():
 v=json.loads(line)
 if 'scan' in v:
  assert v['scan_deadline_seconds']==scan and v['scan']['native_records']==361272 and v['scan']['stub_records']==1232426;scan+=1
 if 'snapshot_durable' in v:assert v['snapshot_durable']['complete_record_restore']=='passed' and len(v['snapshot_durable']['files'])==29;snap.append(v)
assert scan==600 and len(snap)==2
w=load(M/'overlap-witness.json')
for mark in [w['first'],w['last']]:
 c=json.loads(mark['current_bytes']);b=mark['root_bytes'].encode();assert sha(b)==c['root_sha256'] and json.loads(b)['sequence']==mark['sequence']
assert w['last']['sequence']>w['first']['sequence'] and w['last']['observed_ns']>w['first']['observed_ns']
events=[json.loads(l) for l in (M/'common-events.jsonl').read_text().splitlines()];assert all(a['observed_ns']<=b['observed_ns'] for a,b in zip(events,events[1:]));assert any(v['kind']=='snapshot2_durable_sequence_advanced' for v in events)
frames=0
for line in (R/'host-samples.jsonl').read_text().splitlines():
 f=json.loads(line);assert f['rss_bytes']==sum(m[3] for m in f['members']);frames+=1
# Independent current/root/manifest/receipt chain, all unique original receipts.
world=M/'business/world';c=load(world/'CURRENT.json');b=(world/f"root-{c['slot']}.json").read_bytes();assert sha(b)==c['root_sha256'];root=json.loads(b);ref=root['checkpoint_v2'];b=(world/f"world-index-{ref['generation']}.json").read_bytes();assert sha(b)==ref['sha256'];ref=json.loads(b)['receipts'];last=root['sequence'];seen=set()
while ref:
 b=(world/f"receipt-index-{ref['generation']}.json").read_bytes();assert sha(b)==ref['sha256'];idx=json.loads(b)
 b=(world/f"receipts-{ref['generation']}.json").read_bytes();assert len(b)==idx['body_bytes'] and sha(b)==idx['body_sha256'];body=json.loads(b)
 assert body['last']==idx['last']==last and body['first']==idx['first'] and body['previous']==idx['previous'] and body['genesis_sha256']==idx['genesis_sha256']==root['genesis_sha256']
 assert len(body['receipts'])==idx['last']-idx['first']+1 and [r['operation_id'] for r in body['receipts']]==idx['operations']
 for r in body['receipts']:assert 'Ok' in r['outcome'] and r['operation_id'] not in seen;seen.add(r['operation_id'])
 last=idx['first']-1;ref=idx['previous']
assert last==0 and len(seen)==3011
rows={}
for key,bins in hists.items():
 n=samples[key];rank95=(n*95+99)//100;rank99=(n*99+99)//100;total=0;p95=None;p99=None
 for bucket,count in enumerate(bins):
  total+=count
  if p95 is None and total>=rank95:p95=bucket*1000 if bucket<=40000 else maxima[key]
  if p99 is None and total>=rank99:p99=bucket*1000 if bucket<=40000 else maxima[key]
 rows['/'.join(key)]={'samples':n,'p95_conservative_upper_ns':p95,'p99_conservative_upper_ns':p99,'max_ns':maxima[key],'gate':'insufficient' if n<200 else 'passed' if p95<=20000000 and p99<=40000000 else 'failed'}
host_charged=peaks['joint-budget-probe']+65536
assert host_charged<=MiB
composition=charged['sim']+charged['worker']+layout['diagnostics']['charged_bound_bytes']+observer['observer_hot_bound_bytes']+host_charged
result={'schema':'mycity-t13a-budget-audit-v1','status':'same_source_concurrent_diagnostic_completed','process_records':dict(counts),'requested_Rust_heap_peaks_by_role':dict(peaks),'L1_payload_hashes_equal_v40':True,'original_receipts_verified':len(seen),'RSS_frames_checked':frames,'charged_observed_components_bytes':{'sim':charged['sim'],'worker':charged['worker'],'L1':layout['diagnostics']['charged_bound_bytes'],'observer':observer['observer_hot_bound_bytes'],'supervisor_including_tail':host_charged},'component_sum_without_business_driver_diagnostic_allowance':composition,'business_driver_diagnostics_and_native_overhead':'requires separately justified bound; full external owned and full-pressure gate not accepted','raw_bytes':rawbytes,'role_raw_compute':rows,'full_joint_owned':'not_accepted','formal_steady':'not_run','eight_hour':'not_run','real_16GB':'not_run'}
print(json.dumps(result,indent=2))
