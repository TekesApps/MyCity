from pathlib import Path
import json,hashlib,re,datetime
r=Path('/Users/frankqin/dev/MyCity');w=r/'work/water-treatment-runtime-20261025';short=r/'work/w33'
def sha(f):return hashlib.sha256(f.read_bytes()).hexdigest()
def read(n):return json.loads((w/n).read_text())
assert str(w.resolve()).startswith('/Volumes/HP P900/dev/MyCity/work/')
assert str((r/'target').resolve())=='/Volumes/HP P900/dev/MyCity/target'
wip=read('preserved-wip.json');assert len(wip)==24
for n,a in wip.items():assert sha(r/n)==a['sha256'],n
for n,h in read('funding-v2-owner-manifest.json')['files'].items():assert sha(r/n)==h,n
assert sha(r/'crates/city-core/src/facility_work.rs')=='6773e22d14dfd5964a6b5c47e2c4ccdbf9b96fe07ea9f747311689c179122003'
production=[a for a in read('source26-build-inputs.json') if (a['file'].startswith('crates/') and '/src/' in a['file']) or a['file'] in ['Cargo.toml','Cargo.lock','rust-toolchain.toml','.cargo/config.toml']]
for a in production:assert sha(r/a['file'])==a['sha256'],a['file']
latest_tests=[a for a in read('source32-build-inputs.json') if a['file'].startswith(('crates/','tests/fixtures/'))]
for a in latest_tests:assert sha(r/a['file'])==a['sha256'],a['file']
a=read('source26-results.json');b=read('source27-results.json');c=read('source33-result.json')
assert len(a)==33 and len(b)==8 and c['exit_code']==0
for x in a+b+[c]:
 assert x['exit_code']==0,x
 assert sha(Path(x['command'][0]))==x['sha256']
assert sum(x['original_gate'] for x in b)==6 and all(not x['stack_override'] for x in b)
for x in a:
 s=(w/('source26-'+x['package']+'-'+x['target']+'.stdout')).read_text();assert re.search(r'test result: ok\. [1-9][0-9]* passed; 0 failed;',s),x
assert 'test result: ok. 2 passed; 0 failed;' in (w/'source33.stdout').read_text()
water=Path(next(x['evidence_root'] for x in a if x['target']=='water_treatment_store'))
worlds=list(water.glob('water-*/world'));assert len(worlds)==18
cold=[f for f in water.glob('water-*/*-result.json') if 'exit_code' in json.loads(f.read_text())];assert len(cold)==30
for f in cold:
 d=json.loads(f.read_text());assert d['exit_code']==0 and not d['timed_out'];assert (Path(d['config']['world'])/'CURRENT.json').is_file()
res=read('resource30-proof.json');role=read('source33-role-proof.json')
assert res['max_base_plus_private_facility_upper']<=131072 and res['max_engine_plus_source_reserve']<=4194304
assert res['max_source_owned']<=16384 and res['max_source_encoded']<=16384
assert role['balance']['capacity_bytes']==role['balance']['available_touched_bytes']==67108864
assert role['balance']['workspace_grants']==role['balance']['workspace_returns']==3
assert role['balance']['granted_capacity_bytes']==role['balance']['workspace_capacity_bytes']==0
for x in role['events']:assert x['allocation_failures']==0 and x['arena_used_bytes']<=x['arena_usable_bytes'] and x['grant_bytes']==59*1024*1024
assert (short/'water-rundue-37658-1792962507283129000').exists()==False # failed initial attempt remains in original normal32 root
assert (w/'source23-save-failure-diagnosis.json').is_file() and (w/'source33-path-diagnosis.json').is_file()
# Preserve full original bytes; this inventory does not replace any restore data.
manifest=[]
for root in [w,short]:
 for f in sorted(root.rglob('*')):
  if f.is_file() and f.name not in ['acceptance34-verified.json','retained-evidence34-sha.json']:
   manifest.append(dict(file=str(f.resolve()),bytes=f.stat().st_size,sha256=sha(f)))
size=sum(x['bytes'] for x in manifest);assert size<read('artifact-budget.json')['max_bytes']==2147483648
(w/'retained-evidence34-sha.json').write_text(json.dumps(manifest,indent=2)+'\n')
result=dict(observed_at=datetime.datetime.now(datetime.timezone.utc).isoformat(),issue=133,finite_business_contract='all four original133 checklist requirements evidenced; publication/backfill/actual close pending',current_production_files_rechecked=len(production),latest_source32_test_fixture_files_rechecked=len(latest_tests),shared_functional_groups=33,default_stack_groups=8,original_default_groups=6,headless_original_and_pooled_tests=2,groups_overlap_not_unique_business_count=True,current_store_worlds=18,current_store_independent_cold_processes=30,ordinary_save_interrupt_scenarios=8,resource_owned=res,resource_role=role,unrelated_wip_files_preserved=24,original_owner_frozen_files=4,complete_evidence_roots=[str(w.resolve()),str(short.resolve())],retained_manifest='retained-evidence34-sha.json',retained_original_files=len(manifest),retained_logical_bytes=size,evidence_budget_bytes=2147483648,not_accepted=['native/OS/stack full-process peak','global M0/full joint normal64 and diagnostic8','real16GB','sustainable speed/long-term growth','UI/render/GPU','full parent92/sewage/rights/use certification'],pr=None,publication='pending v192; no existing tag substituted for gameplay')
(w/'acceptance34-verified.json').write_text(json.dumps(result,indent=2)+'\n')
print(json.dumps({k:v for k,v in result.items()if k not in ['resource_owned','resource_role']}))
