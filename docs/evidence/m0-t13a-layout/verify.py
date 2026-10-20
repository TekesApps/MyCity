#!/usr/bin/env python3
"""Audit archived full L1 diagnostic; never signs steady/8h/16GB gates."""
import hashlib,json,struct,zipfile
from pathlib import Path
D=Path(__file__).parent
load=lambda n:json.loads((D/n).read_text())
def source(prefix):
    meta=load(prefix+'-source.json');h=hashlib.sha256()
    with zipfile.ZipFile(D/(prefix+'-source.zip')) as z:
        for n in sorted(z.namelist()):
            b=z.read(n);p=n.encode();h.update(struct.pack('<Q',len(p)));h.update(p);h.update(struct.pack('<Q',len(b)));h.update(b)
    assert h.hexdigest()==meta['source_sha256']
    return h.hexdigest()
src=source('diagnostic-10')
root=D.resolve().parents[2]; names=['.cargo/config.toml','Cargo.toml','Cargo.lock','rust-toolchain.toml']
for folder in ['crates','fixtures']:names.extend(str(p.relative_to(root)) for p in (root/folder).rglob('*') if p.is_file())
h=hashlib.sha256()
for n in sorted(names):
 b=(root/n).read_bytes();name=n.encode();h.update(struct.pack('<Q',len(name)));h.update(name);h.update(struct.pack('<Q',len(b)));h.update(b)
assert h.hexdigest()==src, 'verify this frozen evidence in v0.5.32 source checkout'
p=load('diagnostic-10-model.json');o=load('diagnostic-10-observer.json')
assert p['source_sha256']==src and p['scans']==90 and p['all_published_scans_completed']
assert p['diagnostic_outcome']=={'Ok':None} and p['manifest']['normal_joint_budget_bytes']==3328599654
assert len(p['snapshots'])==3 and [s['generation'] for s in p['snapshots']]==[1,2,3]
for s in p['snapshots']:
    assert s['complete_record_restore']=='passed' and s['full_pressure_file_readback']=='passed'
    assert s['durable_complete_seconds']<=60
    assert s['native_restore_bound']['conservative_transient_bound_bytes']<=64*1024**2
assert p['maximum_scan_ready_age_seconds']<=5 and p['maximum_snapshot_start_lateness_seconds']<=5
with zipfile.ZipFile(D/'diagnostic-10-measurements.zip') as z:
    events=[json.loads(b) for b in z.read('model/events.jsonl').splitlines()]
scans=[e['scan'] for e in events if 'scan'in e]
assert len(scans)==90 and scans[-1]['cow_pages']==0
for i,s in enumerate(scans,1):
    assert s['iteration']==i and s['native_records']==361272 and s['stub_records']==1232426 and s['dirty_bytes']==1048576
for item in load('diagnostic-10-payload-audit.json'):
    actual={f['name']:f for f in item['actual_files']}
    for f in item['index']['files']:assert actual[f['name']]==f
assert o['status']=='passed_measurement_wrapper' and o['binary_unchanged'] and o['observer_source_unchanged']
assert o['observer_hot_bound_bytes']<=2*1024**2 and o['combined_diagnostic_hot_bound_bytes']<=8*1024**2
assert o['maximum_observed_swap_net_growth_bytes']<=256*1024**2 and o['maximum_sampled_non_normal_pressure_seconds']<=60
c=load('diagnostic-11-model.json');co=load('diagnostic-11-observer.json')
assert c['restore']['same_source'] and c['restore']['reader_source_sha256']==src
assert c['restore']['restored_generation']==3 and c['restore']['restored_mutation_iteration']==60
assert c['resumed_scan']['iteration']==61 and c['next_generation']['snapshot']['generation']==4
assert c['restore']['all_payload_files_hashed_and_loaded'] and co['status']=='passed_measurement_wrapper'
assert c['next_generation']['snapshot']['complete_record_restore']=='passed'
a=load('diagnostic-10-exact-latencies.json')
assert a['classes']['L1_native_record_index_page']['samples']==4230
assert a['classes']['L1_stub_record_index_page']['samples']==15120
post=load('post-reopen-audit.json');assert post['retained_generations']==[3,4] and post['generation_2_retired'] and post['next_dirty_pattern_independently_recomputed']
assert post['changed_pressure_ranges']==[{'offset':60*1024**2,'bytes':1024**2}]
n=load('negative-observer.json');assert n['model_exit_code']!=0
with zipfile.ZipFile(D/'negative-measurements.zip') as z:assert z.read('model.stderr').strip()==b'layout_restore_checksum:native.json'
assert p['memory_16gb_gate']==c['memory_16gb_gate']=='not_run'
result={'source_sha256':src,'layout_diagnostic':'passed','same_source_cold_seconds':c['restore']['cold_restore_seconds'],'cold_10s_gate':'passed_single_diagnostic' if c['restore']['cold_restore_seconds']<=10 else 'failed_single_diagnostic','steady':'not_run','eight_hour':'not_run','real_16gb':'not_run','full_M0':'not_accepted'}
(D/'checks.json').write_text(json.dumps(result,indent=2)+'\n');print(json.dumps(result,indent=2))
