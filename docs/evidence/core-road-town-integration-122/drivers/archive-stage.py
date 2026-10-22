from pathlib import Path
import json,hashlib,shutil,zipfile,subprocess
r=Path(__file__).resolve().parent;repo=Path('/Users/frankqin/dev/MyCity-core-work/production');dest=repo/'docs/evidence/core-road-town-integration-122'
def sha(p):
 h=hashlib.sha256()
 with p.open('rb') as f:
  for b in iter(lambda:f.read(1048576),b''):h.update(b)
 return h.hexdigest()
def copy(p,q):
 q.parent.mkdir(parents=True,exist_ok=True)
 assert not q.exists(),q
 shutil.copy2(p,q)
dest.mkdir(exist_ok=False);rows=[]
for d in sorted(r.iterdir()):
 if not d.is_dir() or not (d/'exit.json').is_file():continue
 v=json.loads((d/'exit.json').read_text());entry={'run':d.name,'result':v,'external_full_run':str(d),'files':[]}
 for name in ['command.json','exit.json','stdout.log','stderr.log','source-manifest.json','executables.json','cli-executables.json','cli-executable.json','runtime-metrics.json','storage-profile.json']:
  p=d/name
  if p.is_file():copy(p,dest/'runs'/d.name/name);entry['files'].append({'name':name,'size':p.stat().st_size,'sha256':sha(p)})
 if (d/'source.zip').exists():entry['external_source_archive']={'path':str(d/'source.zip'),'sha256':sha(d/'source.zip')}
 rows.append(entry)
for name in ['run-command.py','preserve-executables.py','integrated-cli-driver.py','legacy-reader-driver.py','transport-cli-validator.py','economy-cli-validator.py','archive-stage.py','environment.json','transport-cli-summary.json','integrated-cli-summary.json','advisor-focused-review.md','source-equivalence.json']:
 p=r/name
 if p.is_file():copy(p,dest/'drivers'/name)
for p in r.glob('economy-cli-validator-*.json'):copy(p,dest/'summaries'/p.name)
for path in [r/'68-legacy-reader',r/'70-shared-cache-invalidate',r/'controller-helper-first',r/'controller-helper-first-fixed']:
 for p in sorted(path.rglob('*')):
  if p.is_file() and 'world' not in p.relative_to(path).parts:copy(p,dest/'supplement'/path.name/p.relative_to(path))
# Archive the final verified Rust source, fixtures, lockfile, and actual compile-time doc fixtures.
scopes=['Cargo.toml','Cargo.lock','rust-toolchain.toml','.cargo','crates','fixtures','docs/evidence/core-construction-compat/attempt-01/old-v106-world','docs/evidence/core-construction-compat/attempt-01/old-v106-storage-probe.json','docs/evidence/m0-t13a-r07-independent-arena/legacy-source']
files=set()
for name in scopes:
 p=repo/name
 for f in sorted(p.rglob('*')) if p.is_dir() else [p]:
  if f.is_file():assert not f.is_symlink();files.add(f)
# Bind any other literal compile-time doc fixture actually referenced by source.
import re
for source in (repo/'crates').rglob('*.rs'):
 for literal in re.findall(r'(?:include_bytes|include_str)!\(\s*"([^"]+)"',source.read_text()):
  p=(source.parent/literal).resolve()
  if p.is_file():p.relative_to(repo);files.add(p)
manifest=[{'path':str(p.relative_to(repo)),'size':p.stat().st_size,'sha256':sha(p)} for p in sorted(files)]
(dest/'source-manifest.json').write_text(json.dumps(manifest,indent=2))
with zipfile.ZipFile(dest/'source.zip','x',compression=zipfile.ZIP_DEFLATED) as z:
 for row in manifest:z.write(repo/row['path'],row['path'])
final_test=json.loads((r/'77-headless-cpu-log-tests/source-manifest.json').read_text());test_rows={x['path']:x for x in final_test}
assert all(test_rows[x['path']]==x for x in manifest if x['path'] in test_rows)
assert all((repo/x['path']).is_file() and sha(repo/x['path'])==x['sha256'] for x in final_test if x['path'].startswith(('crates/','fixtures/','.cargo/')) or x['path'] in ['Cargo.toml','Cargo.lock','rust-toolchain.toml'])
prior={x['path']:x for x in json.loads((r/'72-final-core-storage-tests/source-manifest.json').read_text())}
assert all(prior[x['path']]==x for x in manifest if x['path'] in prior and x['path']!='crates/city-headless/src/cpu_log.rs')
index={'schema':'mycity-core-road-town-stage-evidence-v1','status':'functional stage only; no M0 or 16GB acceptance','storage':'HP P900 external USB/APFS SSD','host':'Apple M4 32GiB; not real16GB','runs':rows,'final_source_archive_sha256':sha(dest/'source.zip'),'final_source_manifest_sha256':sha(dest/'source-manifest.json'),'final_rust_and_fixtures_equal_77_test_source':True,'core_storage_runtime_equal_72_test_source':True,'compile_doc_fixtures_included':True,'external_complete_worlds_and_failure_sources_retained':str(r),'fixed_executable_locations':'per-run preserved executable manifests plus old-writer attempt01; retained on external SSD or canonical evidence','cleanup_receipt':'supplement/70-shared-cache-invalidate/receipt.json'}
(dest/'index.json').write_text(json.dumps(index,indent=2));print('archived',len(rows),'runs;',len(manifest),'source files')
