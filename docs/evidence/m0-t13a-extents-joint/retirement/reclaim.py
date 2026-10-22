"""One-shot, exact-file retirement. Never infer deletability from a directory prefix."""
import gzip
import hashlib
import json
import os
import shutil
import stat
import subprocess
import sys
import time
from pathlib import Path

R = Path('/Users/frankqin/dev/MyCity')
D = Path(__file__).resolve().parent
os.chdir(R)

def save(name, value):
    with (D/name).open('x') as f:
        json.dump(value, f, indent=2)
        f.write('\n'); f.flush(); os.fsync(f.fileno())

def sha(p):
    h = hashlib.sha256()
    with Path(p).open('rb') as f:
        for b in iter(lambda: f.read(1024*1024), b''): h.update(b)
    return h.hexdigest()

def meta(p):
    s = Path(p).lstat()
    return [s.st_dev,s.st_ino,s.st_mode,s.st_size,s.st_mtime_ns,s.st_blocks,s.st_nlink]

def identity(m): return m[:5]+m[6:]

def inventory():
    return {str(Path(a)/n):meta(Path(a)/n) for root in ['work','target']
            for a,_,fs in os.walk(root) for n in fs}

def wip():
    names = subprocess.check_output(['git','ls-files','--cached','--others','--exclude-standard'],text=True).splitlines()
    return {p:sha(p) for p in names if Path(p).is_file() and
            (p.startswith('crates/') or p in ['Cargo.toml','Cargo.lock'])}

def occupancy(label):
    ps = subprocess.check_output(['ps','-axo','pid,ppid,etime,command'],text=True)
    (D/(label+'-processes.txt')).write_text('\n'.join(' '.join(f[:3]+[f[3].split()[0]]) for l in ps.splitlines()[1:] if len(f:=l.split(None,3))==4)+'\n')
    for line in ps.splitlines()[1:]:
        fields = line.split(None,3)
        if len(fields)!=4: continue
        exe = Path(fields[3].split()[0]).name.lower()
        if not any(s in exe for s in ['mycity','cargo','rustc']): continue
        q = subprocess.run(['lsof','-a','-p',fields[0],'-d','cwd','-Fn'],capture_output=True,text=True)
        dirs = [l[1:] for l in q.stdout.splitlines() if l.startswith('n')]
        if not dirs:
            assert not subprocess.check_output(['ps','-p',fields[0],'-o','pid='],text=True).strip()
        assert all(Path(d)!=R and R not in Path(d).parents for d in dirs), (line,dirs)
    q = subprocess.run(['lsof','-nP','+D',str(R/'work'),'+D',str(R/'target')],capture_output=True,text=True)
    save(label+'-occupancy.json',dict(code=q.returncode,stdout=q.stdout,stderr=q.stderr))
    assert q.returncode==1 and not q.stdout and not q.stderr, 'in-use or incomplete lsof'
    return dict(time_ns=time.time_ns(),available_bytes=shutil.disk_usage(R).free,
                du_kib={p:int(subprocess.check_output(['du','-sk',p],text=True).split()[0]) for p in ['work','target']})

def protected():
    rows=json.loads(Path('docs/evidence/m0-t13a-r07-report-error/retirement/protected-after.json').read_text())
    assert len(rows)==190
    result=[]
    for x in rows:
        p=x['path']; m=meta(p)
        assert identity(m)==identity(x['metadata']) and sha(p)==x['sha256'],p
        result.append(dict(path=p,metadata=m,sha256=x['sha256']))
    return result

def check_protected(rows):
    for x in rows:
        assert identity(meta(x['path']))==identity(x['metadata']) and sha(x['path'])==x['sha256'],x['path']

def differences(before,after):
    assert before.keys()==after.keys(),'unexpected new/removed work or target file'
    blocks=[]
    for p,m in before.items():
        n=after[p]
        assert identity(m)==identity(n),('retained file changed',p)
        if m[5]!=n[5]: blocks.append(dict(path=p,allocated_delta_bytes=(n[5]-m[5])*512))
    return blocks

if sys.argv[1]=='prepare':
    assert not (D/'plan.json').exists()
    failure=json.loads((D.parent/'controller-failure.json').read_text())
    assert failure['controller_exit']==1 and not failure['performance_acceptance']
    cold=json.loads((D.parent/'postfailure-cold.json').read_text())
    assert len(cold)==2 and {x['generation'] for x in cold}=={1,2}
    assert all(x['exit']==0 and x['cold_reopen_and_scan']=='passed' for x in cold)
    before=occupancy('prepare'); inv=inventory(); pr=protected(); plan=[]; unmatched=[]
    def add(p,why,evidence,**extra):
        m=meta(p)
        assert stat.S_ISREG(m[2]) and m[6]==1 and p.is_relative_to(Path('work'))
        assert str(p) not in {x['path'] for x in pr}
        plan.append(dict(path=str(p),round=p.parts[1],metadata=m,sha256=sha(p),
                         allocated_bytes=m[5]*512,reason=why,retained_evidence=evidence,**extra))
    normal=[('m0-t13a-extents-joint-l1-01',1),('m0-t13a-extents-joint-l1-01',2)]
    for run,generation in normal:
        b=Path('work')/run/'observed/model/layout/snapshots'/f'snapshot-{generation:06}'
        idx=b/'index.json'; index=json.loads(idx.read_text()); idxsha=sha(idx)
        files={x['name']:x for x in index['files']}
        evidence=str(idx)+'; original raw/reports; docs/evidence/m0-t13a-extents-joint; postfailure-cold reports; fixed source/lock/exe'
        if run=='m0-t13a-extents-joint-l1-01':
            for n in ['pressure.bin','work-reserve.bin']:
                p=b/n; assert files[n]['bytes']==p.stat().st_size and files[n]['sha256']==sha(p)
                add(p,'terminal rebuildable synthetic load; actual independent cold validation; no next-stage original-path dependency',evidence,index=str(idx),index_sha256=idxsha,directly_restorable_after=False)
        else:
            assert not (b/'pressure.bin').exists() and not (b/'work-reserve.bin').exists()
        # Other snapshot components are deliberately outside this release.
        # Retain them pending the retry's component archive comparison.
    assert len(plan)==4
    assert len({(x['metadata'][0],x['metadata'][1]) for x in plan})==len(plan)
    with gzip.open(D/'inventory-before.json.gz','wt') as f:json.dump(inv,f,separators=(',',':'))
    save('protected-before.json',pr);save('wip-before.json',wip());save('plan.json',plan)
    save('retained-unmatched.json',unmatched);save('prepare.json',before)
    print(json.dumps(dict(files=len(plan),allocated_bytes=sum(x['allocated_bytes'] for x in plan),before=before),indent=2))
elif sys.argv[1]=='execute':
    assert (D/'advisor-review.md').is_file() and not (D/'delete-intent.json').exists()
    plan=json.loads((D/'plan.json').read_text());pr=json.loads((D/'protected-before.json').read_text())
    inv=json.loads(gzip.open(D/'inventory-before.json.gz','rt').read());wp=json.loads((D/'wip-before.json').read_text())
    block_before=differences(inv,inventory());check_protected(pr);assert wip()==wp
    for x in plan:
        assert identity(meta(x['path']))==identity(x['metadata']) and sha(x['path'])==x['sha256']
        assert sha(x['index'])==x['index_sha256']
        if 'archive' in x:assert sha(x['archive'])==x['archive_sha256']
    before=occupancy('immediate');save('delete-intent.json',plan)
    for x in plan:
        p=Path(x['path']);m=meta(p);assert identity(m)==identity(x['metadata'])
        x['metadata_at_unlink']=m;x['allocated_bytes']=m[5]*512;p.unlink()
    after=occupancy('after');post=inventory();gone={x['path'] for x in plan}
    block_after=differences({p:m for p,m in inv.items() if p not in gone},post)
    check_protected(pr);assert wip()==wp
    save('deleted.json',plan);save('protected-after.json',[dict(x,metadata=meta(x['path'])) for x in pr])
    save('retained-block-changes.json',dict(before=block_before,after=block_after,excluded_from_deleted_total=True))
    save('availability.json',dict(not_directly_restorable=sorted({str(Path(x['path']).parent) for x in plan}),
        complete_original_files_retained=190,pending_is_not_valid_restore_point=True,
        original_pressure_not_archived=True,component_gzip_is_not_complete_snapshot=True))
    result=dict(files_deleted=len(plan),allocated_bytes_removed=sum(x['allocated_bytes'] for x in plan),
        before=before,after=after,filesystem_available_delta_bytes=after['available_bytes']-before['available_bytes'],
        protected_files_sha_verified=len(pr),source_wip_unchanged=True,other_files_identity_metadata_unchanged=len(post),
        new_archive_bytes=0,new_target=False,build_or_model_run=False,
        attribution='inode-deduplicated st_blocks; not APFS exclusive physical bytes. df observational only; retained block changes excluded.')
    save('result.json',result);print(json.dumps(result,indent=2))
