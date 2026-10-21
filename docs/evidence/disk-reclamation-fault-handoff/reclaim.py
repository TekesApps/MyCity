"""One-shot retirement of nine explicitly released synthetic fault payloads.
No directory deletion, no source/build edits, no pressure rebuilding.
"""
from pathlib import Path
import gzip
import hashlib
import json
import os
import stat
import subprocess
import time

R=Path('/Users/frankqin/dev/MyCity');D=Path(__file__).resolve().parent
os.chdir(R)
def save(name, value):
    with (D/name).open('w') as f:
        json.dump(value,f,indent=2);f.write('\n');f.flush();os.fsync(f.fileno())
    fd=os.open(D,os.O_RDONLY);os.fsync(fd);os.close(fd)
def meta(p):
    s=p.lstat();return [s.st_dev,s.st_ino,s.st_size,s.st_mtime_ns,s.st_blocks*512,s.st_nlink]
def inventory():
    return {str(Path(d)/n):meta(Path(d)/n) for d,_,fs in os.walk('work') for n in fs}
def sha(p):
    h=hashlib.sha256()
    with p.open('rb') as f:
        for b in iter(lambda:f.read(4*1024**2),b''):h.update(b)
    return h.hexdigest()
def sources():
    names=subprocess.check_output(['git','ls-files','--cached','--others','--exclude-standard','-z'],text=True).split('\0')
    return {n:sha(Path(n)) for n in names if n and not n.startswith('docs/') and n!='README.md' and Path(n).is_file()}
def metrics():
    s=os.statvfs(R)
    return dict(time_ns=time.time_ns(),work_du_kib=int(subprocess.check_output(['du','-sk','work'],text=True).split()[0]),target_du_kib=int(subprocess.check_output(['du','-sk','target'],text=True).split()[0]),filesystem_available_bytes=s.f_bavail*s.f_frsize)
def idle(label):
    ps=subprocess.check_output(['ps','-axo','pid,ppid,comm'],text=True);(D/(label+'-processes.txt')).write_text(ps)
    assert not any(line.split()[-1].endswith('mycity-headless') for line in ps.splitlines() if line.split()),'live model'
    p=subprocess.run(['lsof','-nP','-Fpcfn'],capture_output=True,text=True);assert p.returncode==0,p.stderr
    hits=[];pid=command=fd=''
    for line in p.stdout.splitlines():
        if line.startswith('p'):pid=line[1:]
        elif line.startswith('c'):command=line[1:]
        elif line.startswith('f'):fd=line[1:]
        elif line.startswith('n') and line[1:].startswith((str(R/'work'),str(R/'target'))):
            hits.append(dict(pid=pid,command=command,fd=fd,path=line[1:]))
    save(label+'-occupancy.json',hits);assert not hits,hits

LATEST=Path('work/report-lifetime-formal-l1-01/observed/model/layout/snapshots')
PROTECTED=[LATEST/'snapshot-000001',LATEST/'snapshot-000002.pending',
    Path('work/steady-controller-formal-l2-01/observed/model/layout/snapshots/snapshot-000007'),
    Path('work/steady-controller-formal-l2-01/observed/model/layout/snapshots/snapshot-000008'),
    Path('work/t13a50-joint-01/model/layout/snapshots/snapshot-000001'),
    Path('work/steady-controller-smoke-l2-01/observed/model/layout/snapshots/snapshot-000001')]
def protected():
    out={}
    for root in PROTECTED:
        assert root.is_dir(),str(root)
        out[str(root)]={str(p):dict(metadata=meta(p),sha256=sha(p)) for p in sorted(root.rglob('*')) if p.is_file()}
        if not root.name.endswith('.pending'):
            idx=json.loads((root/'index.json').read_text())
            for row in idx['files']:
                item=out[str(root)][str(root/row['name'])]
                assert item['metadata'][2]==row['bytes'] and item['sha256']==row['sha256']
    return out
def equal(old, reference, prefix=False):
    a=old.stat();b=reference.stat()
    assert a.st_size<=b.st_size if prefix else a.st_size==b.st_size
    h=hashlib.sha256()
    with old.open('rb') as x,reference.open('rb') as y:
        for block in iter(lambda:x.read(4*1024**2),b''):
            assert block==y.read(len(block)),str(old)
            h.update(block)
        if not prefix:assert not y.read(1)
    return h.hexdigest()

assert not (D/'deleted.json').exists(),'one-shot; do not rerun'
assert json.loads((D/'cold.json').read_text())['exit_code']==0
assert json.loads((D/'cold-report.json').read_text())['cold_reopen_and_scan']=='passed'
assert (D/'advisor-review.md').exists(),'independent dependency handoff required'
idle('prepare');save('source-before.json',sources());before_inventory=inventory()
with gzip.open(D/'work-before.json.gz','wt') as f:json.dump(before_inventory,f)
save('protected-before.json',protected())
items=[(Path('work/layout-region-l1-smoke-01/observed/model/layout/snapshots/snapshot-000001.pending/pressure.bin'),LATEST/'snapshot-000001/pressure.bin',True)]
for name in ['causal-clock-formal-l1-01','snapshot-backing-formal-l1-01']:
    base=Path('work')/name/'observed/model/layout/snapshots'
    for generation in ['snapshot-000001','snapshot-000002.pending']:
        for filename in ['pressure.bin','work-reserve.bin']:
            items.append((base/generation/filename,LATEST/generation/filename,False))
plan=[]
for old,reference,prefix in items:
    m=meta(old);assert stat.S_ISREG(old.lstat().st_mode) and m[5]==1
    digest=equal(old,reference,prefix)
    index=old.parent/'index.json'
    if index.exists():
        row=next(r for r in json.loads(index.read_text())['files'] if r['name']==old.name)
        assert row['bytes']==m[2] and row['sha256']==digest
    plan.append(dict(path=str(old),round=old.parts[1],generation=old.parent.name,
        metadata=m,sha256=digest,equivalent_to=str(reference),byte_comparison='exact prefix' if prefix else 'complete byte-for-byte',
        reason='Old synthetic load no longer required after actual latest full cold restore and independent handoff; old source/exe/raw/index/components/phase evidence retained',
        evidence='docs/evidence/disk-reclamation-fault-handoff/cold-report.json; original round docs/evidence and work raw files',
        directly_restorable_after=False,full_fault_payload_preserved_after=False))
assert len(plan)==9 and len({tuple(r['metadata'][:2]) for r in plan})==9
save('plan.json',plan);assert sources()==json.loads((D/'source-before.json').read_text())
idle('execute');before=metrics();save('delete-before-metrics.json',before)
removed=[]
for row in plan:
    p=Path(row['path']);assert meta(p)==row['metadata'];p.unlink();removed.append(row);save('deleted.json',removed)
for parent in sorted({str(Path(r['path']).parent) for r in plan}):
    fd=os.open(parent,os.O_RDONLY);os.fsync(fd);os.close(fd)
after=metrics();save('delete-after-metrics.json',after)
now=inventory();deleted={r['path'] for r in plan};assert set(before_inventory)-set(now)==deleted
assert not set(now)-set(before_inventory)
allocation=[]
for p,m in now.items():
    old=before_inventory[p]
    assert all(m[i]==old[i] for i in (0,1,2,3,5)),p
    if m[4]!=old[4]:allocation.append(dict(path=p,before=old[4],after=m[4]))
save('retained-allocation-only-differences.json',allocation)
after_protected=protected();save('protected-after.json',after_protected)
prior=json.loads((D/'protected-before.json').read_text())
for root,rows in after_protected.items():
    for p,row in rows.items():assert row['sha256']==prior[root][p]['sha256'],p
assert sources()==json.loads((D/'source-before.json').read_text());save('source-after.json',sources())
save('availability.json',[dict(path=r['path'],round=r['round'],generation=r['generation'],directly_restorable=False,full_old_fault_payload_preserved=False,latest_originals=str(LATEST),evidence=r['evidence']) for r in plan])
save('result.json',dict(before=before,after=after,deleted_files=len(plan),deleted_single_link_inode_allocated_bytes=sum(r['metadata'][4] for r in plan),target_deleted_bytes=0,new_pressure_archive_bytes=0,retained_files=len(now),retained_content_identity_length_mtime_links_unchanged=True,retained_allocation_differences=len(allocation),observed_filesystem_available_delta_bytes=after['filesystem_available_bytes']-before['filesystem_available_bytes'],filesystem_delta_attributed_to_cleanup_bytes=None,accounting='st_blocks is not exclusive physical APFS blocks; df influenced by shared blocks/snapshots/other tasks; do not credit historical reclamations',source_wip_preserved=True))
print((D/'result.json').read_text(),flush=True)
