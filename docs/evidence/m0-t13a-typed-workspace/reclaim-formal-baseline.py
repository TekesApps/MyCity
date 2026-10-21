"""One-shot retirement of four released synthetic payloads, never directories."""
import gzip
import hashlib
import json
import os
import shutil
import stat
import subprocess
from pathlib import Path

D = Path(__file__).resolve().parent/'formal-retirement'
R = D.parents[3]
assert R == Path('/Users/frankqin/dev/MyCity')
D.mkdir(exist_ok=True)
assert not (D/'before.json').exists(), 'one-shot; re-audit instead of rerunning'

def sha(path):
    h = hashlib.sha256()
    with path.open('rb') as f:
        for chunk in iter(lambda:f.read(1024*1024),b''):
            h.update(chunk)
    return h.hexdigest()

def inventory():
    out = []
    for parent,dirs,files in os.walk(R/'work',followlinks=False):
        for name in files:
            path=Path(parent)/name;s=path.lstat()
            out.append([str(path.relative_to(R)),s.st_dev,s.st_ino,s.st_mode,s.st_size,s.st_mtime_ns,s.st_nlink,s.st_blocks])
    return sorted(out)

def metrics():
    return dict(du_kib={n:int(subprocess.check_output(['du','-sk',str(R/n)],text=True).split()[0]) for n in ('work','target')},
                filesystem_available_bytes=shutil.disk_usage(R).free)

def occupancy(label):
    result=subprocess.run(['lsof','-nP','+D',str(R/'work'),'+D',str(R/'target')],text=True,capture_output=True)
    (D/(label+'-open-files.txt')).write_text(result.stdout)
    (D/(label+'-open-files.stderr')).write_text(result.stderr)
    assert result.returncode==1 and not result.stdout and not result.stderr
    process=subprocess.check_output(['ps','-axo','pid=,ppid=,comm='],text=True)
    relevant=[line for line in process.splitlines() if any(x in line.split(None,2)[-1].lower() for x in ('mycity-headless','/cargo','/rustc'))]
    (D/(label+'-processes.json')).write_text(json.dumps(relevant,indent=2)+'\n')
    assert not relevant,relevant

info=json.loads((D.parent/'source.json').read_text())
assert sha(Path(info['binary']))==info['binary_sha256']
assert json.loads((D.parent/'regression-terminal.json').read_text())['all_commands_exit_zero']
assert json.loads((D.parent/'formal-result.json').read_text())['current_two_full_cold_restores'].startswith('passed')
protected=[]
for generation in (7,8):
    root=R/f'work/typed-workspace-formal-l1-01/observed/model/layout/snapshots/snapshot-{generation:06}'
    index=json.loads((root/'index.json').read_text())
    assert len(index['files'])==29
    for entry in index['files']:
        path=root/entry['name'];assert path.stat().st_size==entry['bytes'] and sha(path)==entry['sha256']
        protected.append(dict(path=str(path.relative_to(R)),bytes=entry['bytes'],sha256=entry['sha256']))

deleted=[]
for generation in (7,8):
    root=R/f'work/steady-controller-formal-l2-01/observed/model/layout/snapshots/snapshot-{generation:06}'
    index=json.loads((root/'index.json').read_text())
    for name in ('pressure.bin','work-reserve.bin'):
        path=root/name;s=path.lstat();entry=next(e for e in index['files'] if e['name']==name)
        assert stat.S_ISREG(s.st_mode) and s.st_nlink==1 and path.resolve()==path
        assert s.st_size==entry['bytes'] and sha(path)==entry['sha256']
        deleted.append(dict(path=str(path.relative_to(R)),run='steady-controller-formal-l2-01',generation=generation,
                            dev=s.st_dev,inode=s.st_ino,bytes=s.st_size,mtime_ns=s.st_mtime_ns,
                            allocated_bytes=s.st_blocks*512,links=s.st_nlink,sha256=entry['sha256'],
                            reason='pure synthetic payload; replaced by actual current7/8 full recovery and advisor dependency release',
                            retained_evidence='original index, typed components, L2 business/WAL, raw observer/model logs and fixed source/exe remain; docs/evidence/m0-t13a-typed-workspace/advisor-formal-review.md'))
occupancy('before')
rows=inventory();before=metrics()
(D/'before.json').write_text(json.dumps(dict(metrics=before,deleted=deleted,protected=protected),indent=2)+'\n')
with gzip.open(D/'inventory-before.json.gz','wt') as f:json.dump(rows,f,separators=(',',':'))
occupancy('immediate-before-delete')
for row in deleted:
    path=R/row['path'];s=path.lstat()
    assert (s.st_dev,s.st_ino,s.st_size,s.st_mtime_ns,s.st_nlink)==(row['dev'],row['inode'],row['bytes'],row['mtime_ns'],1)
    path.unlink()
for directory in {str((R/row['path']).parent) for row in deleted}:
    fd=os.open(directory,os.O_RDONLY);os.fsync(fd);os.close(fd)
selected={row['path'] for row in deleted}
after_rows=inventory();unchanged_before=[row[:7] for row in rows if row[0] not in selected]
assert unchanged_before==[row[:7] for row in after_rows]
for row in protected:assert sha(R/row['path'])==row['sha256']
occupancy('after')
after=metrics()
availability=[dict(path=row['path'],state='retired; not directly fully restorable',reason=row['reason']) for row in deleted]
(D/'availability.json').write_text(json.dumps(availability,indent=2)+'\n')
(D/'deleted.json').write_text(json.dumps(deleted,indent=2)+'\n')
result=dict(before=before,after=after,deleted_files=len(deleted),allocated_bytes_removed=sum(row['allocated_bytes'] for row in deleted),
            available_delta_bytes=after['filesystem_available_bytes']-before['filesystem_available_bytes'],
            attribution='st_blocks sum is not APFS exclusive physical space; df change is separate, never fully credited',
            other_work_file_identity_size_mtime_links_unchanged=True,protected_current_files_rehashed=len(protected),
            shared_target_preserved=True,source_exe_logs_business_wal_preserved=True)
(D/'result.json').write_text(json.dumps(result,indent=2)+'\n');print(json.dumps(result,indent=2),flush=True)
