"""Terminal stage only. Exact old-v70 and synthetic-input retirement; no glob delete."""
from pathlib import Path
import gzip,hashlib,json,os,subprocess,time,zipfile
R=Path('/Users/frankqin/dev/MyCity');D=Path(__file__).resolve().parent/'retention'
D.mkdir();os.chdir(R)
def save(name, value):
    with (D / name).open('w') as f:
        f.write(json.dumps(value, indent=2) + '\n')
        f.flush(); os.fsync(f.fileno())
    fd = os.open(str(D), os.O_RDONLY)
    try: os.fsync(fd)
    finally: os.close(fd)

def sha(p):
    h = hashlib.sha256()
    with Path(p).open('rb') as f:
        for b in iter(lambda: f.read(4 * 1024**2), b''): h.update(b)
    return h.hexdigest()

def meta(p):
    s = Path(p).lstat()
    return [s.st_dev, s.st_ino, s.st_size, s.st_mtime_ns, s.st_blocks * 512, s.st_nlink]

def inventory():
    return {str(Path(d) / n): meta(Path(d) / n)
            for d, _, fs in os.walk('work') for n in fs}

def metrics():
    s = os.statvfs(R)
    return dict(time_ns=time.time_ns(),
        work_du_kib=int(subprocess.check_output(['du', '-sk', 'work'], text=True).split()[0]),
        target_du_kib=int(subprocess.check_output(['du', '-sk', 'target'], text=True).split()[0]),
        filesystem_available_bytes=s.f_bavail * s.f_frsize)

def idle(label):
    # comm avoids capturing other tasks' full command arguments/secrets.
    ps = subprocess.check_output(['ps', '-axo', 'pid,ppid,lstart,comm'], text=True)
    (D / (label + '-processes.txt')).write_text(ps)
    p = subprocess.run(['lsof', '-nP', '-Fpcfn'], capture_output=True, text=True)
    assert p.returncode == 0, p.stderr
    hits = []; pid = cmd = fd = ''
    for line in p.stdout.splitlines():
        if line.startswith('p'): pid = line[1:]
        elif line.startswith('c'): cmd = line[1:]
        elif line.startswith('f'): fd = line[1:]
        elif line.startswith('n') and str(R) in line:
            hits.append(dict(pid=pid, command=cmd, fd=fd, path=line[1:]))
    save(label + '-occupancy.json', hits)
    assert not any(h['path'].startswith((str(R / 'work'), str(R / 'target'))) for h in hits)
    assert not any(line.split()[-1].endswith('mycity-headless') for line in ps.splitlines() if line.split())


def protected():
 out=[]
 for rel in ['work/t13a85-joint-01/model/layout/snapshots/snapshot-000001','work/t13a85-joint-01/model/layout/snapshots/snapshot-000002','work/t13a50-joint-01/model/layout/snapshots/snapshot-000001']:
  root=Path(rel);idx=json.loads((root/'index.json').read_text())
  for e in idx['files']:
   p=root/e['name'];assert p.stat().st_size==e['bytes'] and sha(p)==e['sha256'],str(p)
  out.append(dict(path=rel,indexed_files=len(idx['files']),all_original_bytes_verified=True))
 return out
assert json.loads((D.parent/'joint-terminal.json').read_text())['same_source_full_joint_smoke_completed']
assert (D.parent/'advisor-final-review.md').exists()
idle('before');before=metrics();files=inventory()
with gzip.open(D/'work-before.json.gz','wt') as f:json.dump(files,f)
save('protected-before.json',protected())
source_files=json.loads((D.parent/'arena-freeze.json').read_text())
with zipfile.ZipFile(D.parent/'arena-source.zip') as z:
 source={n:sha(n) for n in z.namelist()};assert all(sha(n)==hashlib.sha256(z.read(n)).hexdigest() for n in z.namelist())
source['AGENTS.md']=sha('AGENTS.md')
save('source-before.json',source)
plan=[]
for generation in [1,2]:
 root=Path(f'work/t13a70-joint-01/model/layout/snapshots/snapshot-{generation:06}')
 idx=json.loads((root/'index.json').read_text())
 for name in ['pressure.bin','work-reserve.bin']:
  p=root/name;e=next(x for x in idx['files'] if x['name']==name)
  assert p.stat().st_size==e['bytes'] and sha(p)==e['sha256']
  assert p.is_file() and not p.is_symlink() and meta(p)[5]==1
  plan.append(dict(path=str(p),round='t13a70-joint-01',metadata=meta(p),sha256=e['sha256'],reason='Superseded complete recovery baseline: both new v85 generations exist and latest actual cold restore passed; v70 audit depends only on retained index. No remaining old-pressure restore dependency.',evidence='docs/evidence/m0-t13a-role-arena/advisor-final-review.md; work/t13a70-joint-01/model/layout/snapshots retained index; full new v85 originals',directly_restorable_after=False))
input_manifest=json.loads((D.parent/'store-boundary-input.json').read_text());root=Path(input_manifest['synthetic_world'])
assert {str(p.relative_to(root)) for p in root.rglob('*') if p.is_file()}==set(input_manifest['files'])
for name,e in input_manifest['files'].items():
 p=root/name;assert p.stat().st_size==e['bytes'] and sha(p)==e['sha256']
 assert not p.is_symlink() and meta(p)[5]==1
 plan.append(dict(path=str(p),round='role-arena-store-boundary',metadata=meta(p),sha256=e['sha256'],reason='Synthetic near16MiB CP copy after old/new actual probes terminal and independent review; no downstream restore dependency; full v81 base unchanged and retained.',evidence='docs/evidence/m0-t13a-role-arena/store-boundary-input.json; store-boundary-proof.json; probe stdout/stderr and generator',directly_restorable_after=False))
base=Path(input_manifest['base']);assert input_manifest['base_sha256']=={str(p.relative_to(base)):sha(p) for p in base.rglob('*') if p.is_file()}
save('plan.json',plan)
assert len(plan)==13
for x in plan:assert meta(x['path'])==x['metadata'] and sha(x['path'])==x['sha256']
idle('immediate');removed=[]
for x in plan:
 assert meta(x['path'])==x['metadata'];Path(x['path']).unlink();removed.append(x);save('deleted.json',removed)
for parent in sorted({str(Path(x['path']).parent) for x in removed}):
 fd=os.open(parent,os.O_RDONLY)
 try:os.fsync(fd)
 finally:os.close(fd)
after=metrics();now=inventory();deleted={x['path'] for x in removed}
assert all(p not in now for p in deleted)
assert all(now.get(p)==m for p,m in files.items() if p not in deleted)
assert source=={n:sha(n) for n in source}
save('source-after.json',dict(unchanged=True,files=len(source)))
save('protected-after.json',protected())
save('result.json',dict(before=before,after=after,deleted_files=len(removed),deleted_allocated_bytes=sum(x['metadata'][4] for x in removed),retained_existing_files_unchanged=len(files)-len(removed),observed_df_available_delta=after['filesystem_available_bytes']-before['filesystem_available_bytes'],target_deleted_bytes=0,new_archive_bytes=0,accounting='Single-link inode allocation; APFS exclusive reclaim not measured. df independent. No historical cleanup recounted.',retired_availability='Old v70 two full layouts and synthetic input cannot directly restore. New v85 two generations and old v50 original remain complete; all old indexes, business, logs and components kept.'))
print((D/'result.json').read_text())
