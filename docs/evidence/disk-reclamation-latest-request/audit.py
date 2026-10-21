"""Read-only current artifact audit; never unlink or change work/target."""
import gzip
import hashlib
import json
import os
from pathlib import Path
import subprocess
import time

R = Path('/Users/frankqin/dev/MyCity')
D = Path(__file__).resolve().parent
os.chdir(R)

def save(name, value):
    (D / name).write_text(json.dumps(value, indent=2) + '\n')

def inventory(root):
    rows = {}
    for base, dirs, files in os.walk(root, followlinks=False):
        for name in files:
            p = Path(base) / name
            s = p.lstat()
            rows[str(p)] = [s.st_dev, s.st_ino, s.st_size, s.st_mtime_ns,
                            s.st_blocks * 512, s.st_nlink, s.st_mode]
    return rows

def metrics():
    s = os.statvfs(R)
    return dict(time_ns=time.time_ns(), filesystem_available_bytes=s.f_bavail*s.f_frsize,
                work_du_kib=int(subprocess.check_output(['du','-sk','work'], text=True).split()[0]),
                target_du_kib=int(subprocess.check_output(['du','-sk','target'], text=True).split()[0]))

def idle(label):
    ps = subprocess.check_output(['ps','-axo','pid,ppid,comm'], text=True)
    (D / (label+'-processes.txt')).write_text(ps)
    assert not any(line.split()[-1].endswith(('mycity-headless','cargo','rustc'))
                   for line in ps.splitlines() if line.split()), 'build/model active; audit deferred'
    p = subprocess.run(['lsof','-nP','-Fpcfn'], capture_output=True, text=True)
    assert p.returncode == 0, p.stderr
    hits=[]; pid=command=fd=''
    for line in p.stdout.splitlines():
        if line.startswith('p'): pid=line[1:]
        elif line.startswith('c'): command=line[1:]
        elif line.startswith('f'): fd=line[1:]
        elif line.startswith('n') and line[1:].startswith((str(R/'work')+'/',str(R/'target')+'/')):
            hits.append(dict(pid=pid,command=command,fd=fd,path=line[1:]))
    save(label+'-occupancy.json',hits)
    assert not hits, 'work/target held; audit deferred'

assert not (D/'result.json').exists(), 'one-shot audit evidence; do not overwrite'
idle('before')
before=metrics()
work=inventory('work'); target=inventory('target')
with gzip.open(D/'inventory-before.json.gz','wt') as f:
    json.dump(dict(work=work,target=target),f)
large=[]
for p,m in work.items():
    if Path(p).name not in ('pressure.bin','work-reserve.bin'): continue
    if 'causal-clock-formal-l1-01' in p:
        reason='Current repeated-snapshot allocation failure: keep complete first snapshot and entire second pending for diagnosis/recovery; pending is not a restore point.'
    elif 'layout-region-l1-smoke-01' in p:
        reason='Prior actual allocation failure pending: still required fault comparison, incomplete, not restorable.'
    elif 'steady-controller-formal-l2-01' in p:
        reason='Latest published full recovery baseline generations 7/8; replacement formal run failed, dependency not released.'
    elif 'steady-controller-smoke-l2-01' in p:
        reason='Warmup failure comparison; retain until diagnosis dependency explicitly released.'
    elif 't13a50-joint-01' in p:
        reason='Old-format full cold-restore compatibility input; retain until compatibility dependency explicitly released.'
    else:
        reason='Unknown dependency; retain pending individual review.'
    large.append(dict(path=p,metadata=m,reason=reason,
                      evidence=str(Path(p).parent),deleted=False))
save('retained-large-payloads.json',large)
save('deleted.json',[])
idle('after')
after_work=inventory('work'); after_target=inventory('target')
assert work==after_work and target==after_target, 'artifact metadata changed during read-only audit'
after=metrics()
unique={tuple(m[:2]):m for m in work.values()}
save('result.json',dict(before=before,after=after,deleted_files=0,
    deleted_allocated_bytes=0,new_work_archive_bytes=0,target_deleted_bytes=0,
    work_files=len(work),work_unique_inodes=len(unique),
    work_unique_inode_allocated_bytes=sum(m[4] for m in unique.values()),
    pressure_files=sum(Path(p).name=='pressure.bin' for p in work),
    reserve_files=sum(Path(p).name=='work-reserve.bin' for p in work),
    pressure_allocated_bytes=sum(m[4] for p,m in work.items() if Path(p).name=='pressure.bin'),
    timing_sample_allocated_bytes=sum(m[4] for p,m in work.items() if Path(p).name=='compute-durations.bin'),
    all_work_target_metadata_unchanged=True,
    observed_filesystem_available_delta_bytes=after['filesystem_available_bytes']-before['filesystem_available_bytes'],
    filesystem_delta_attributed_to_cleanup_bytes=0,
    accounting='No deletion this audit. st_blocks is not exclusive APFS physical space. Historical deletions and concurrent filesystem changes are not credited.',
    preserved='source/WIP, complete recovery inputs, all fault pending/negative fixtures, raw business/WAL/receipts, timing samples, fixed source/dependencies/executables; shared target reused'))
print((D/'result.json').read_text())
