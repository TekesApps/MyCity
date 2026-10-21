#!/usr/bin/env python3
"""Offline bindings for original record components; never full pressure acceptance."""
import hashlib,json,sys,zipfile
from pathlib import Path
D=Path(__file__).resolve().parent;R=D.parents[2];run=R/'work'/sys.argv[1]
load=lambda p:json.loads(p.read_text());sha=lambda b:hashlib.sha256(b).hexdigest()
info=load(D/'source.json');freeze=load(D/'freeze.json');index=load(D/'original-index.json')
raw=(D/'original-index.json').read_bytes();assert sha(raw)==freeze['index_sha256']
with zipfile.ZipFile(R/'docs/evidence/m0-t13a-joint-endurance/t13a50-joint-01/artifacts.zip') as z:
 assert z.read('model/layout/snapshots/snapshot-000001/index.json')==raw
report=load(run/'model/report.json');observer=load(run/'observer-report.json')
assert observer['status']=='passed_measurement_wrapper' and observer['model_exit_code']==0
assert observer['binary_unchanged'] and observer['observer_source_unchanged']
assert observer['binary_sha256']==info['binary_sha256'] and observer['observer_source_sha256']==info['observer_sha256']
assert report['source_sha256']==info['source_sha256']
assert report['original_index_sha256']==freeze['index_sha256'] and report['producer_source_sha256']==freeze['producer_source_sha256']
a=report['all_original_component_files'];expected=freeze['record_component_files']
assert len(a)==len(expected)==27 and len({x['name'] for x in a})==27
expected={x['name']:x for x in expected}
for item in a:
 assert item['canonical_roundtrip_equal'] and {k:item[k] for k in ('name','bytes','sha256')}==expected[item['name']]
 path=Path(freeze['snapshot'])/item['name'];h=hashlib.sha256();n=0
 with path.open('rb') as f:
  while b:=f.read(65536):h.update(b);n+=len(b)
 assert h.hexdigest()==item['sha256'] and n==item['bytes']
assert report['native_records']==361272 and report['stub_records']==1232426
bound=report['native_restore_bound']
assert bound['decoded_owned_bytes']==index['manifest']['native']['owned_capacity_bytes']
assert bound['largest_replacement_bytes']==0 and bound['parser_io_stack_slack_bytes']==6*1024**2
assert bound['conservative_transient_bound_bytes']==bound['decoded_owned_bytes']+6*1024**2<=64*1024**2
assert report['full_layout']==report['steady']==report['eight_hour']==report['real16gb']=='not_run'
assert report['joint_normal64']==report['common8']=='not_accepted' and report['full_M0']=='not_accepted'
assert 0<report['actual_seconds']<=observer['actual_wall_seconds']
heaps=[json.loads(x) for x in (run/'model/heaps.jsonl').read_text().splitlines()]
assert len(heaps)==1 and heaps[0]['role']=='layout-record-restore' and heaps[0]['success'] and heaps[0]['source_sha256']==info['source_sha256']
print(json.dumps({'source_sha256':info['source_sha256'],'old_index_bound_to_v50_archive':True,'all27_original_component_files_and_roundtrip_hashes':True,'record_bytes':sum(x['bytes'] for x in a),'native_records':report['native_records'],'stub_records':report['stub_records'],'native_bound':bound,'actual_component_seconds':report['actual_seconds'],'scope':'all original record components only; no pressure/reserve image; not a full layout/joint-memory or speed acceptance','full_layout':'not_run','normal64':'not_accepted','common8':'not_accepted','steady':'not_run','eight_hour':'not_run','real16gb':'not_run','full_M0':'not_accepted'},indent=2))
