"""Read completed frozen evidence; never starts/restarts a model or deletes artifacts."""
from pathlib import Path
import collections,hashlib,json,struct,zipfile
D=Path(__file__).resolve().parent;R=D.parents[2]
def read(name):return json.loads((D/name).read_text())
def save(name,value):(D/name).write_text(json.dumps(value,indent=2)+'\n')
terminal=read('metadata-terminal.json');freeze=read('metadata-freeze.json');h=hashlib.sha256()
with zipfile.ZipFile(D/'metadata-source.zip') as z:
 for name in sorted(z.namelist()):
  b=(R/name).read_bytes();assert b==z.read(name),name;nb=name.encode()
  h.update(struct.pack('<Q',len(nb)));h.update(nb);h.update(struct.pack('<Q',len(b)));h.update(b)
assert h.hexdigest()==freeze['source_sha256']==terminal['source_sha256']
for name in ['metadata-workspace-final','metadata-clippy-final','metadata-fmt-final','metadata-build-final','metadata-compat-compile','metadata-compat-run','metadata-full-business','metadata-new-writer-compile','metadata-new-writer-run','metadata-old-reader-new-output','metadata-new-reader-new-output']:
 assert read(name+'.json')['exit_code']==0,name
heaps=[json.loads(line) for line in (D/'metadata-full-rust-heaps.jsonl').read_text().splitlines()]
assert all(x['source_sha256']==freeze['source_sha256'] and x['success'] for x in heaps)
roles=collections.defaultdict(list)
for x in heaps:roles[x['role']].append(x)
save('heap-summary.json',dict(records=len(heaps),roles={role:dict(records=len(rows),max_lifetime_requested_bytes=max(x['heap']['lifetime_peak_requested_bytes'] for x in rows),max_requested_plus_reserved_tail_bytes=max(x['heap']['lifetime_peak_requested_bytes']+x['tail_reserved_bytes'] for x in rows)) for role,rows in roles.items()},scope='Per-process Rust heap lifetime maxima; not simultaneous totals, broker grants, RSS, physical memory, stack, mapped or foreign memory; tail reserved not exact exit peak'))
assert len(roles['compute-worker-bounded'])==724
business=read('metadata-full-business-report.json')
assert business['completed_hour_targets']==720 and business['verified_drive_targets']==723 and business['actual_worker_windows']==724
assert business['complete_final_business_and_air']=='passed' and business['oldest_receipt_retry']=='passed_without_side_effect'
fixed=read('metadata-source.json');exe=R/fixed['binary'];assert hashlib.sha256(exe.read_bytes()).hexdigest()==fixed['binary_sha256']
save('source-terminal-integrity.json',dict(source_sha256=freeze['source_sha256'],frozen_files=freeze['source_files'],source_and_lock_unchanged=True,fixed_binary_sha256=fixed['binary_sha256'],actual_full_business_verified=True,old_reader_and_new_writer_verified=True,normal_pool_grant_accepted=False,formal_joint_performance_accepted=False,eight_hour_accepted=False,real_16gb='not_run',full_m0='not_accepted'))
print(json.dumps(dict(source=freeze['source_sha256'],heap_records=len(heaps),terminal='all listed actual commands exit0'),indent=2))
