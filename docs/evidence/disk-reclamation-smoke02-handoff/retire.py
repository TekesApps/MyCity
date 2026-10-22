"""One-shot retirement of four explicitly superseded synthetic payloads only."""
import hashlib,json,os,stat,subprocess,time
from pathlib import Path

R=Path('/Users/frankqin/dev/MyCity')
D=Path(__file__).resolve().parent
OLD=R/'work/j2s-01/observed/model/layout/snapshots'
NEW=R/'work/j3s-01/observed/model/layout/snapshots'
PROTECTED=R/'docs/evidence/work-migration-20261022/attempt03/protected-after.json'

def save(name,value):
    (D/name).write_text(json.dumps(value,indent=2)+'\n')

def sha(path):
    h=hashlib.sha256()
    with path.open('rb') as stream:
        for b in iter(lambda:stream.read(8*1024**2),b''):h.update(b)
    return h.hexdigest()

def identity(path):
    s=path.stat()
    return [s.st_dev,s.st_ino,s.st_mode,s.st_size,s.st_mtime_ns,s.st_blocks,s.st_nlink]

def protected_check(label):
    rows=json.loads(PROTECTED.read_text())
    assert len(rows)==190
    for i,row in enumerate(rows):
        p=R/row['path']
        assert identity(p)==row['metadata'],(label,'protected metadata',row['path'])
        assert sha(p)==row['sha256'],(label,'protected hash',row['path'])
        if (i+1)%50==0:print(label,'protected verified',i+1,flush=True)
    save('protected-'+label+'.json',{'count':len(rows),'all_original_identities_and_full_sha256':'equal_current_protected_manifest','manifest':str(PROTECTED),'manifest_sha256':sha(PROTECTED)})

def occupancy(label):
    ps=subprocess.check_output(['ps','-axo','pid=,comm='],text=True)
    relevant=[]
    for line in ps.splitlines():
        pid_s,comm=line.strip().split(None,1)
        leaf=Path(comm).name
        if leaf not in {'cargo','rustc','mycity-headless','town_transport','town_market','payroll_exit'}:continue
        proc=subprocess.run(['lsof','-a','-p',pid_s,'-d','cwd','-Fn'],capture_output=True,text=True)
        if proc.returncode==1 and not proc.stdout:continue  # Process already exited.
        assert proc.returncode==0 and not proc.stderr,(label,'cwd check failed',pid_s,proc.stderr)
        cwd=[s[1:] for s in proc.stdout.splitlines() if s.startswith('n')]
        assert len(cwd)==1,(label,'cwd unavailable',pid_s)
        if leaf in {'mycity-headless','town_transport','town_market','payroll_exit'} or cwd[0].startswith(('/Users/frankqin/dev/MyCity','/Volumes/HP P900/dev/MyCity')):
            relevant.append({'pid':int(pid_s),'executable':comm,'cwd':cwd[0]})
    opened=subprocess.run(['lsof','-nP','+D',str(OLD.resolve(strict=True))],capture_output=True,text=True)
    save('occupancy-'+label+'.json',{'relevant_processes':relevant,'lsof_exit':opened.returncode,'lsof_stdout':opened.stdout,'lsof_stderr':opened.stderr,'resolved_old_scope':str(OLD.resolve())})
    assert not relevant,(label,'MyCity still running',relevant)
    assert opened.returncode==1 and not opened.stdout and not opened.stderr,(label,'occupied or incomplete lsof',opened.stdout,opened.stderr)

def free_bytes():
    s=os.statvfs(R/'work');return s.f_bavail*s.f_frsize

assert not (D/'plan.json').exists(),'one-shot plan already exists; do not blindly repeat'
assert (R/'work').is_symlink() and str((R/'work').resolve())=='/Volumes/HP P900/dev/MyCity/work'
old_report=json.loads((R/'work/j2s-01/observed/observer-report.json').read_text())
new_report=json.loads((R/'work/j3s-01/observed/observer-report.json').read_text())
for report in (old_report,new_report):
    assert report['status']=='passed_measurement_wrapper' and report['model_exit_code']==0
    assert report['binary_sha256']=='59aa365ed46b5d373218fcbcf1d34f930d674d15607ed510190361891439b4e0'
for run in ('j2s-01','j3s-01'):
    cold=json.loads((R/'work'/run/'observed/model/layout-restored/report.json').read_text())
    assert cold['cold_reopen_and_scan']=='passed'
occupancy('before')
print('terminal processes and old-scope handles checked',flush=True)
protected_check('before')
protected={str((R/row['path']).resolve()) for row in json.loads(PROTECTED.read_text())}
rows=[]
for generation in (1,2):
    name=f'snapshot-{generation:06d}'
    older=json.loads((OLD/name/'index.json').read_text())
    newer=json.loads((NEW/name/'index.json').read_text())
    for index in (older,newer):
        assert index['generation']==generation and index['manifest']['source_sha256']=='32c6e61b9565545177c330f34455611b329fbe1cb38591ec5f6e7879731c4da2'
    # Verify every replacement file, retaining the full files, not hashes alone.
    for item in newer['files']:
        p=NEW/name/item['name']
        assert p.is_file() and not p.is_symlink() and p.stat().st_size==item['bytes']
        assert sha(p)==item['sha256'],('replacement',str(p))
    for leaf in ('pressure.bin','work-reserve.bin'):
        prior=next(x for x in older['files'] if x['name']==leaf)
        after=next(x for x in newer['files'] if x['name']==leaf)
        assert prior==after,('not duplicate',name,leaf)
        p=OLD/name/leaf
        assert str(p.resolve()) not in protected and not p.is_symlink() and stat.S_ISREG(p.stat().st_mode)
        ident=identity(p)
        assert ident[6]==1,'shared inode; allocation credit requires separate analysis'
        assert ident[3]==prior['bytes'] and sha(p)==prior['sha256']
        rows.append({'path':str(p),'identity':ident,'sha256':prior['sha256'],'allocated_bytes':ident[5]*512,'replacement':str(NEW/name/leaf),'generation':generation,'reason':'older successful same-source synthetic payload is byte-identical to a complete retained replacement; latest replacement cold-reopened successfully','retained_evidence':'raw/index/log/latency/business remain under work/j2s-01; replacement originals under work/j3s-01; execution summaries in docs/evidence/m0-t13a-joint-smoke-control-02 and -03'})
    print('replacement and retirement payload verified generation',generation,flush=True)
assert len(rows)==4
occupancy('final')
before_free=free_bytes()
save('plan.json',{'files':rows,'allocated_bytes_to_retire':sum(x['allocated_bytes'] for x in rows),'protected_originals':190,'retains_current_complete_recovery_originals':True,'old_snapshots_after_retirement':'not_directly_fully_restorable','free_bytes_before_unlink':before_free})
with (D/'deleted.jsonl').open('x') as log:
    for row in rows:
        p=Path(row['path'])
        assert identity(p)==row['identity'] and p.resolve().is_relative_to(OLD.resolve())
        p.unlink()
        log.write(json.dumps(row)+'\n');log.flush();os.fsync(log.fileno())
for generation in (1,2):
    save_path=OLD/f'snapshot-{generation:06d}'/'RETIREMENT.json'
    save_path.write_text(json.dumps({'status':'synthetic_pressure_and_work_reserve_retired_not_directly_fully_restorable','retired_files':['pressure.bin','work-reserve.bin'],'replacement':str(NEW/f'snapshot-{generation:06d}'),'retirement_evidence':str(D)},indent=2)+'\n')
after_free=free_bytes()
protected_check('after')
save('result.json',{'deleted_files':len(rows),'deleted_file_allocated_bytes':sum(x['allocated_bytes'] for x in rows),'free_bytes_before_unlink':before_free,'free_bytes_after_unlink':after_free,'observed_available_space_delta_bytes':after_free-before_free,'available_delta_is_not_exclusive_reclamation_credit':True,'old_snapshots':'not_directly_fully_restorable','current_j3s_snapshots':'complete_originals_retained_and_fully_hashed','protected_190_originals':'full_SHA_and_identities_unchanged','target':'not_modified'})
print('retired four exact older synthetic payloads; protected 190 and complete replacements retained',flush=True)
