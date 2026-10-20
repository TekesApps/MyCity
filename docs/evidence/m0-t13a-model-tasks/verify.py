#!/usr/bin/env python3
"""Read raw capsules and recompute compatibility; never promotes synthetic/full CPU to Sim."""
import hashlib,json,struct,zipfile
from pathlib import Path
D=Path(__file__).resolve().parent;R=D.parents[2];sha=lambda b:hashlib.sha256(b).hexdigest()
info=json.loads((D/'source.json').read_text());h=hashlib.sha256()
with zipfile.ZipFile(D/'source.zip') as z:
 for name in sorted(z.namelist()):
  b=z.read(name);assert b==(R/name).read_bytes(),name;n=name.encode();h.update(struct.pack('<Q',len(n)));h.update(n);h.update(struct.pack('<Q',len(b)));h.update(b)
 assert sha(z.read('fixtures/m0-t13a-acceptance.json'))=='876ea995ea7834bfcea59b2ef8fa56d502aed9006477bcc67bcaa3f5cd944508'
assert h.hexdigest()==info['source_sha256']
assert sha((R/'target/release/mycity-headless').read_bytes())==info['binary_sha256']
assert sha((D/'functional.py').read_bytes())==info['functional_harness_sha256']
assert sha((D/'source-support.zip').read_bytes())==info['source_support_sha256']
with zipfile.ZipFile(D/'source-support.zip') as support:
 assert len(support.namelist())==1
 for name in support.namelist():assert support.read(name)==(R/name).read_bytes()
for stem in ['source-initial','source-second']:
 metadata=json.loads((D/(stem+'.json')).read_text());digest=hashlib.sha256()
 with zipfile.ZipFile(D/(stem+'.zip')) as archive:
  for name in sorted(archive.namelist()):
   data=archive.read(name);encoded=name.encode();digest.update(struct.pack('<Q',len(encoded)));digest.update(encoded);digest.update(struct.pack('<Q',len(data)));digest.update(data)
  assert len(archive.namelist())==metadata['files']
 assert digest.hexdigest()==metadata['source_sha256']
packets={}
for folder in ['initial-failed','second-wire-pass','final-functional','validation']:
 rows=json.loads((D/folder/'manifest.json').read_text())
 with zipfile.ZipFile(D/folder/'artifacts.zip') as z:
  assert set(z.namelist())=={r['path'] for r in rows}
  for r in rows:
   b=z.read(r['path']);assert len(b)==r['bytes'] and sha(b)==r['sha256']
 packets[folder]=len(rows)
with zipfile.ZipFile(R/'docs/evidence/m0-t13a-process-instances/t13a3c-instances-01/artifacts.zip') as old:
 expected_targets=old.read('model/cpu-targets.jsonl');expected_engine=old.read('model/cpu-final-engine.json');expected_report=json.loads(old.read('model/cpu-report.json'))
 for folder,capsule in [('initial-failed','source-initial.json'),('second-wire-pass','source-second.json'),('final-functional','source.json')]:
  with zipfile.ZipFile(D/folder/'artifacts.zip') as z:
   actual=json.loads(z.read('model/cpu-report.json'));c=json.loads((D/capsule).read_text());assert actual['source_sha256']==c['source_sha256']
   assert z.read('model/cpu-final-engine.json')==expected_engine
   assert [json.loads(x) for x in z.read('model/cpu-targets.jsonl').splitlines()]==[json.loads(x) for x in expected_targets.splitlines()]
   a=dict(actual);b=dict(expected_report)
   for k in ['source_sha256','work_wall_seconds']:a.pop(k);b.pop(k)
   if folder=='initial-failed':
    assert a['field']['maximum']==2.000018 and b['field']['maximum']==2.0000178813934326
    a['field']['maximum']=b['field']['maximum']
   assert a==b
   if folder=='final-functional':
    proof=json.loads(z.read('verification.json'));assert proof['source_sha256']==info['source_sha256'] and proof['binary_sha256']==info['binary_sha256']
    records=[json.loads(s) for s in z.read('heaps.jsonl').splitlines()];assert len(records)==8 and all(r['source_sha256']==info['source_sha256'] for r in records)
    assert len({(r['pid'],r['instance_unix_ns']) for r in records})==8
    result=json.loads(z.read('model/compare/result.json'));assert result['business_sha256']==actual['business_sha256']
    assert proof['performance_gate']==proof['eight_hour_gate']==proof['memory_16gb_gate']=='not_run'
with zipfile.ZipFile(D/'validation/artifacts.zip') as z:
 focused=z.read('tests-final-source.txt').decode();assert '5 passed; 0 failed' in focused and '1 passed; 0 failed' in focused and 'test result: FAILED' not in focused
 clippy=z.read('clippy-final-fixed.txt').decode();assert 'Finished' in clippy and 'error:' not in clippy
 assert 'test result: FAILED' not in z.read('tests-workspace.txt').decode()
print(json.dumps({'source_sha256':info['source_sha256'],'packets':packets,'full_CPU_723_targets_and_engine_exact_v42':True,'final_report_fields_match_except_source_and_wall':True,'initial_failure_preserved':True,'synthetic_comparison_only':True,'final_source_focused_tests':6,'full_Sim_this_source':'not_run','eight_hour_gate':'not_run','memory_16gb_gate':'not_run','full_M0':'not_accepted'},indent=2))
