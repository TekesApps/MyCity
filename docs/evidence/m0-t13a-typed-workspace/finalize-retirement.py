"""Finish metadata/evidence after conservative post-delete process assertion.
NO deletion operations. Never rerun the one-shot retirement procedure.
"""
import gzip
import hashlib
import json
import os
import shutil
import subprocess
from pathlib import Path

D=Path(__file__).resolve().parent/'formal-retirement'
R=D.parents[3]
assert not (D/'result.json').exists()
before=json.loads((D/'before.json').read_text())
with gzip.open(D/'inventory-before.json.gz','rt') as f:rows=json.load(f)
deleted=before['deleted'];selected={row['path'] for row in deleted}
assert len(selected)==4 and all(not (R/name).exists() for name in selected)
after_rows=[]
for parent,dirs,files in os.walk(R/'work',followlinks=False):
    for name in files:
        path=Path(parent)/name;s=path.lstat()
        after_rows.append([str(path.relative_to(R)),s.st_dev,s.st_ino,s.st_mode,s.st_size,s.st_mtime_ns,s.st_nlink])
assert sorted(after_rows)==[row[:7] for row in rows if row[0] not in selected]
for row in before['protected']:
    h=hashlib.sha256()
    with (R/row['path']).open('rb') as f:
        for chunk in iter(lambda:f.read(1024*1024),b''):h.update(chunk)
    assert h.hexdigest()==row['sha256']
opened=subprocess.run(['lsof','-nP','+D',str(R/'work'),'+D',str(R/'target')],text=True,capture_output=True)
(D/'final-open-files.txt').write_text(opened.stdout)
(D/'final-open-files.stderr').write_text(opened.stderr)
assert opened.returncode==1 and not opened.stdout and not opened.stderr
after=dict(du_kib={name:int(subprocess.check_output(['du','-sk',str(R/name)],text=True).split()[0]) for name in ('work','target')},
           filesystem_available_bytes=shutil.disk_usage(R).free)
availability=[dict(path=row['path'],state='retired; not directly fully restorable',reason=row['reason']) for row in deleted]
(D/'availability.json').write_text(json.dumps(availability,indent=2)+'\n')
(D/'deleted.json').write_text(json.dumps(deleted,indent=2)+'\n')
result=dict(before=before['metrics'],after=after,deleted_files=4,
            allocated_bytes_removed=sum(row['allocated_bytes'] for row in deleted),
            available_delta_bytes=after['filesystem_available_bytes']-before['metrics']['filesystem_available_bytes'],
            attribution='st_blocks is not APFS exclusive physical space; df change separately observed, not fully credited',
            other_work_file_identity_size_mtime_links_unchanged=True,protected_current_files_rehashed=len(before['protected']),
            shared_target_preserved=True,source_exe_logs_business_wal_preserved=True,
            original_script_exit=1,original_script_stop='post-delete assertion saw unattributed 1.95 Cargo process after successful metadata and hash verification; no project file occupancy',
            finalization='no additional deletion; full other-work metadata and protected SHA checks repeated; directory occupancy empty')
(D/'result.json').write_text(json.dumps(result,indent=2)+'\n')
print(json.dumps(result,indent=2),flush=True)
