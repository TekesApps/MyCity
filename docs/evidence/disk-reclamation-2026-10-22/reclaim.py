"""Explicit old component retirement. prepare first; execute only reviewed plan.
No directory/glob removal, no model run, no cache clearing, no regeneration.
"""
from pathlib import Path
import gzip, hashlib, json, os, stat, subprocess, sys, time

R = Path('/Users/frankqin/dev/MyCity')
D = Path(__file__).resolve().parent
os.chdir(R)
ROOTS = [
    't13a1b-layout-full-01', 't13a1b-layout-full-04', 't13a1b-layout-full-10',
    't13a3a-joint-01', 't13a3b-budget-01', 't13a3b-budget-02',
    't13a3g-joint-01', 't13a3h-joint-01', 't13a60-joint-01',
    't13a62-joint-01', 't13a63-joint-01', 't13a64-joint-01',
    't13a65-joint-01', 't13a65-joint-02',
]
CANON = Path('work/t13a70-joint-01/model/layout/snapshots/snapshot-000002')
ARCHIVE = Path('work/retained-component-evidence/duplicate-stubs-v1')
# Explicit synthetic L1 stub fields, never real business state.
NAMES = tuple(x + '.json' for x in ('maps','households','housing','visitors','operators','entities','equity_links','general_contracts','od','path_refs','cargo_batches','passenger_batches','bus_train_vehicles','ship_aircraft_vehicles','car_vehicles','matters','team_members','analysis_tasks','hosts','teaching_batches','current_services','cases','bills'))

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

def equal(a, b):
    with Path(a).open('rb') as x, Path(b).open('rb') as y:
        while True:
            bx = x.read(4 * 1024**2); by = y.read(4 * 1024**2)
            assert bx == by, (a, b)
            if not bx: return

def durable_archive(plan):
    for record in plan['retained'].values():
        p = Path(record['archive'])
        with p.open('rb') as f: os.fsync(f.fileno())
        h = hashlib.sha256(); total = 0
        with gzip.open(p, 'rb') as src:
            for b in iter(lambda: src.read(1024**2), b''):
                total += len(b); h.update(b)
        assert total == record['bytes'] and h.hexdigest() == record['sha256']
    for p in (ARCHIVE, ARCHIVE.parent, ARCHIVE.parent.parent):
        fd = os.open(str(p), os.O_RDONLY)
        try: os.fsync(fd)
        finally: os.close(fd)

def verify_protected():
    roots = [CANON, CANON.with_name('snapshot-000001'),
        Path('work/t13a50-joint-01/model/layout/snapshots/snapshot-000001')]
    out = []
    for root in roots:
        idx = json.loads((root / 'index.json').read_text())
        for row in idx['files']:
            p = root / row['name']
            assert p.stat().st_size == row['bytes'] and sha(p) == row['sha256'], str(p)
        out.append(dict(path=str(root), indexed_files=len(idx['files']), full_sha256_verified=True))
    return out

if sys.argv[1] == 'prepare':
    assert not (D / 'plan.json').exists(), 'never overwrite a reviewed plan'
    idle('prepare')
    save('prepare-metrics.json', metrics())
    files = inventory()
    with gzip.open(D / 'work-before.json.gz', 'wt') as f: json.dump(files, f)
    save('protected-before.json', verify_protected())
    ARCHIVE.mkdir(parents=True, exist_ok=True)
    retained = {}
    for n in NAMES:
        p = CANON / n; a = ARCHIVE / (n + '.gz')
        assert not a.exists()
        with p.open('rb') as src, gzip.open(a, 'wb', compresslevel=6) as dst:
            for b in iter(lambda: src.read(1024**2), b''): dst.write(b)
        h = hashlib.sha256()
        with gzip.open(a, 'rb') as src, p.open('rb') as original:
            for b in iter(lambda: src.read(1024**2), b''):
                assert b == original.read(len(b)); h.update(b)
            assert original.read(1) == b''
        assert h.hexdigest() == sha(p)
        retained[n] = dict(canonical=str(p), archive=str(a), sha256=sha(p),
                           archive_sha256=sha(a), bytes=p.stat().st_size,
                           archive_allocated_bytes=meta(a)[4])
    plan = []
    exclusions = []
    for root in ROOTS:
        for n in NAMES:
            for p in sorted((Path('work') / root).rglob(n)):
                # Only already-incomplete, normal snapshot components. Fault and
                # shared-inode inputs are retained, regardless of matching hash.
                if p.parent.name.endswith('.pending') or meta(p)[5] != 1:
                    exclusions.append(str(p)); continue
                assert p.parent.name.startswith('snapshot-')
                assert not (p.parent / 'pressure.bin').exists(), str(p)
                assert stat.S_ISREG(p.lstat().st_mode)
                if sha(p) != retained[n]['sha256']:
                    exclusions.append(str(p)); continue
                equal(p, CANON / n)
                idx = json.loads((p.parent / 'index.json').read_text())
                assert idx['manifest']['layout_not_business'] is True
                e = next(x for x in idx['files'] if x['name'] == n)
                assert e['sha256'] == retained[n]['sha256'] and e['bytes'] == p.stat().st_size
                plan.append(dict(path=str(p), round=root, metadata=meta(p),
                    snapshot=str(p.parent), original_index=str(p.parent / 'index.json'),
                    original_index_sha256=sha(p.parent / 'index.json'),
                    sha256=retained[n]['sha256'], retained=retained[n],
                    reason='Retired pressure layout; byte-identical immutable component retained in full canonical and lossless archive; no active restore input.',
                    directly_restorable_after=False))
    proposal = dict(files=plan, excluded=exclusions, retained=retained,
        planned_allocated_bytes=sum(x['metadata'][4] for x in plan))
    durable_archive(proposal)
    save('plan.json', proposal)
    print(json.dumps(dict(planned_files=len(plan), allocated_bytes=sum(x['metadata'][4] for x in plan), exclusions=exclusions)))
elif sys.argv[1] == 'execute':
    assert (D / 'advisor-review.md').exists(), 'review must precede deletion'
    idle('execute')
    plan = json.loads((D / 'plan.json').read_text())
    durable_archive(plan)
    with gzip.open(D / 'work-before.json.gz', 'rt') as f: old = json.load(f)
    protected = verify_protected()
    for n, r in plan['retained'].items():
        assert sha(r['canonical']) == r['sha256']
        assert sha(r['archive']) == r['archive_sha256']
    for row in plan['files']:
        p = Path(row['path']); assert meta(p) == row['metadata']
        assert sha(row['original_index']) == row['original_index_sha256']
        assert sha(p) == row['sha256']; equal(p, row['retained']['canonical'])
    # Recheck process/handles immediately before mutations, after potentially long hashing.
    for p in D.iterdir():
        if p.is_file():
            with p.open('rb') as f: os.fsync(f.fileno())
    fd = os.open(str(D), os.O_RDONLY)
    try: os.fsync(fd)
    finally: os.close(fd)
    idle('immediate')
    before = metrics(); removed = []
    for row in plan['files']:
        assert meta(row['path']) == row['metadata']
        Path(row['path']).unlink(); removed.append(row)
        save('deleted.json', removed)
    for parent in sorted({str(Path(x['path']).parent) for x in removed}):
        fd = os.open(parent, os.O_RDONLY)
        try: os.fsync(fd)
        finally: os.close(fd)
    after = metrics(); now = inventory(); deleted = {x['path'] for x in removed}
    assert all(p not in now for p in deleted)
    assert all(now.get(p) == m for p, m in old.items() if p not in deleted), 'retained work changed'
    assert verify_protected() == protected
    save('protected-after.json', protected)
    ids = {(x['metadata'][0], x['metadata'][1]): x['metadata'][4] for x in removed}
    gross = sum(ids.values()); archive_bytes = sum(x['archive_allocated_bytes'] for x in plan['retained'].values())
    result = dict(before=before, after=after, deleted_files=len(removed),
        removed_unique_inodes=len(ids), deleted_allocated_bytes=gross,
        retained_archive_allocated_bytes=archive_bytes,
        net_allocated_reduction_including_archive=gross-archive_bytes,
        df_available_delta_observed=after['filesystem_available_bytes']-before['filesystem_available_bytes'],
        retained_existing_work_files_unchanged=len(old)-len(removed),
        retained_archive_new_files=list(str(ARCHIVE / (n+'.gz')) for n in NAMES),
        target_deleted_bytes=0, independent_target_created=False,
        accounting='st_blocks inode-deduplicated allocation is not APFS exclusive physical reclaimed space. df delta includes unrelated activity; not wholly attributable. Prior deletions excluded.')
    save('result.json', result); print(json.dumps(result, indent=2))
else: raise SystemExit('prepare or execute required')
