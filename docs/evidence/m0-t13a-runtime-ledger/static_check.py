#!/usr/bin/env python3
import collections,hashlib,json,struct,zipfile
from pathlib import Path
D=Path(__file__).resolve().parent;R=D.parents[2];sha=lambda b:hashlib.sha256(b).hexdigest();info=json.loads((D/'source.json').read_text());h=hashlib.sha256()
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
p=D/'t13a3f-full-01';rows=json.loads((p/'manifest.json').read_text())
with zipfile.ZipFile(p/'artifacts.zip') as z:
 assert set(z.namelist())=={r['path'] for r in rows}
 for row in rows:
  b=z.read(row['path']);assert len(b)==row['bytes'] and sha(b)==row['sha256']
 observer=json.loads(z.read('observer-report.json'));report=json.loads(z.read('model/report.json'))
 assert observer['status']=='passed_measurement_wrapper'
 assert observer['binary_sha256']==info['binary_sha256'] and report['source_sha256']==info['source_sha256']
 assert observer['observer_source_sha256']==info['observer_sha256']
 proof=json.loads((D/'full-verification.json').read_text());assert proof['source_sha256']==info['source_sha256']
 assert proof['process_records']=={'scale-sim-probe':1,'sim-host-scale-full':2,'compute-worker-bounded':724,'scale-model-task':3}
 coverage=json.loads((D/'process-coverage.json').read_text());frames=collections.Counter();last=None
 for line in z.read('host-samples.jsonl').splitlines():
  last=json.loads(line)
  for pid in {row[0] for row in last['members']}:frames[pid]+=1
 for mode in ['reference','before-reopen','compare']:
  helper=json.loads(z.read('model/'+mode+'/result.json'));entry=coverage['helper_coverage'][mode]
  assert entry['pid']==helper['pid'] and entry['sampled_frames']==frames[helper['pid']]
 assert coverage['last_frame_member_roles']==[row[4] for row in last['members']]
 assert set(coverage['last_frame_member_roles'])=={'observer','sampler_ps'}
readiness=json.loads((D/'readiness.json').read_text());samples=readiness['samples'];assert len(samples)==25 and all(s['pressure']==1 for s in samples)
span=samples[-1]['elapsed_seconds']-samples[0]['elapsed_seconds'];gap=max(b['elapsed_seconds']-a['elapsed_seconds'] for a,b in zip(samples,samples[1:]));assert span>=120 and gap<6
peaks=proof['requested_Rust_heap_role_lifetime_peaks_bytes'];driver_fee=peaks['scale-sim-probe']+65536;assert driver_fee<=1048576
print(json.dumps({'source_sha256':info['source_sha256'],'binary_sha256':info['binary_sha256'],'source_files_verified':info['source_files'],'archived_files_verified':len(rows),'all_payload_archived':True,'normal_readiness_sample_span_seconds':span,'readiness_maximum_sample_gap_seconds':gap,'driver_whole_process_heap_peak_plus_tail_bytes':driver_fee,'candidate_driver_1MiB_bound_met_in_this_run':True,'shared_diagnostic_8MiB_gate':'not_accepted','global_normal_64MiB_gate':'not_accepted','formal_steady':'not_run','eight_hour':'not_run','real_16GB':'not_run'},indent=2))
