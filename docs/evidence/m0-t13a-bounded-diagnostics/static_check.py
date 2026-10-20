#!/usr/bin/env python3
"""Audit frozen sources and archived bytes; large L1 data is separately retained."""
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
files=0
for folder in ['t13a3g-model-tasks-functional','t13a3g-joint-01']:
 p=D/folder;rows=load(p/'manifest.json')
 with zipfile.ZipFile(p/'artifacts.zip') as z:
  assert set(z.namelist())=={r['path'] for r in rows}
  for row in rows:
   b=z.read(row['path']);assert len(b)==row['bytes'] and sha(b)==row['sha256'];files+=1
  if folder.endswith('joint-01'):
   o=json.loads(z.read('observer-report.json'))
   assert o['status']=='failed' and o['reason']=='system non-normal pressure exceeded 60 seconds' and o['maximum_sampled_non_normal_pressure_seconds']>60
   assert o['binary_sha256']==info['binary_sha256'] and o['observer_source_sha256']==info['observer_sha256'] and o['binary_unchanged'] and o['observer_source_unchanged']
   assert 'model/report.json' not in z.namelist() and 'model/heaps.jsonl' not in z.namelist()
   events=[json.loads(s) for s in z.read('model/common-events.jsonl').splitlines()];assert [v['kind'] for v in events]==['supervisor_started','layout_ready_observed']
   frames=[json.loads(s) for s in z.read('host-samples.jsonl').splitlines()];assert len(frames)==245 and all(v['rss_bytes']==sum(m[3] for m in v['members']) for v in frames)
  else:assert json.loads(z.read('verification.json'))['source_sha256']==info['source_sha256']
readiness=load(D/'readiness.json')['samples'];assert len(readiness)==25 and all(r['pressure']==1 for r in readiness)
span=readiness[-1]['elapsed_seconds']-readiness[0]['elapsed_seconds'];gap=max(b['elapsed_seconds']-a['elapsed_seconds'] for a,b in zip(readiness,readiness[1:]));assert span>=120 and gap<6
print(json.dumps({'source_sha256':info['source_sha256'],'binary_sha256':info['binary_sha256'],'source_files_verified':info['source_files'],'archived_files_verified':files,'normal_readiness_span_seconds':span,'readiness_max_gap_seconds':gap,'large_L1_pressure_payload':'retained_in_ignored_work_with_original_snapshot_index_hashes; not_embedded_in_git','full_joint_owned':'not_accepted','formal_steady':'not_run','four_speeds':'not_run','eight_hour':'not_run','real_16GB':'not_run'},indent=2))
