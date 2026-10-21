"""Exact-source regression and original full business diagnostic. Not formal performance.
One shared target, one retained fixed executable, no new L1 pressure copies.
"""
import hashlib, json, os, struct, subprocess, time, zipfile
from pathlib import Path
D = Path(__file__).resolve().parent
R = D.parents[2]
os.chdir(R)
def save(n, v):
    p = D/n; tmp = p.with_suffix(p.suffix+'.tmp')
    tmp.write_text(json.dumps(v, indent=2)+'\n'); tmp.replace(p)
def sha(p): return hashlib.sha256(Path(p).read_bytes()).hexdigest()
files = sorted(['.cargo/config.toml','Cargo.toml','Cargo.lock','rust-toolchain.toml']+
    [p.relative_to(R).as_posix() for root in ['crates','fixtures'] for p in (R/root).rglob('*') if p.is_file()])
def digest():
    h = hashlib.sha256()
    for n in files:
        b = (R/n).read_bytes(); name = n.encode()
        h.update(struct.pack('<Q',len(name))); h.update(name)
        h.update(struct.pack('<Q',len(b))); h.update(b)
    return h.hexdigest()
assert not (D/'arena-source.zip').exists(), 'never overwrite a frozen run'
source = digest()
with zipfile.ZipFile(D/'arena-source.zip','w',compression=zipfile.ZIP_DEFLATED) as z:
    for n in files: z.write(R/n,n)
save('arena-freeze.json',dict(source_sha256=source,source_files=len(files),
    lock_sha256=sha('Cargo.lock'),fixture_sha256=sha('fixtures/m0-t13a-acceptance.json'),
    opening_sha256=sha('fixtures/m0-t13a-full-opening-candidate.json'),
    source_png_sha256=sha('fixtures/terrain/m0-flat16.png'),source_png_bytes=207,
    source_png_dimensions=[257,257],source_png_bit_depth=16,
    controller_pid=os.getpid(),started_unix_ns=time.time_ns(),new_target=False))
def run(name, cmd, env=None):
    assert digest()==source, 'source changed during validation'
    t = time.monotonic()
    with (D/(name+'.stdout')).open('x') as o,(D/(name+'.stderr')).open('x') as e:
        p = subprocess.Popen(cmd,cwd=R,env=env,stdout=o,stderr=e)
        save('arena-running.json',dict(controller_pid=os.getpid(),pid=p.pid,name=name,command=cmd,source_sha256=source))
        code=p.wait()
    assert digest()==source, 'source changed during validation'
    fact=dict(command=cmd,exit_code=code,seconds=time.monotonic()-t,source_sha256=source)
    save(name+'.json',fact); print(name,code,fact['seconds'],flush=True)
    if code: raise SystemExit(code)
    return fact
for name, cmd in [
    ('workspace-final',['cargo','test','--workspace','--release','--locked','--offline']),
    ('clippy-final',['cargo','clippy','--workspace','--all-targets','--release','--locked','--offline','--','-D','warnings']),
    ('fmt-final',['cargo','fmt','--all','--','--check']),
    ('build-final',['cargo','build','--workspace','--release','--locked','--offline','--message-format=json'])]:run(name,cmd)
fixed=R/'work/candidate-binaries/role-arena/mycity-headless'
fixed.parent.mkdir(parents=True,exist_ok=True)
assert not fixed.exists(); exe=R/'target/release/mycity-headless'
assert source.encode() in exe.read_bytes(); os.link(exe,fixed)
save('arena-source.json',dict(source_sha256=source,source_files=len(files),binary=str(fixed.relative_to(R)),
    binary_sha256=sha(fixed),lock_sha256=sha('Cargo.lock'),scope='Whole Rust role fixed arena diagnostic; formal joint performance and 8h not accepted'))
libs={}
for line in (D/'build-final.stdout').read_text().splitlines():
    x=json.loads(line)
    if x.get('reason')=='compiler-artifact' and x['target']['name'] in ['mycity_core','mycity_storage','mycity_map','mycity_role_allocator','serde_json']:
        libs[x['target']['name']]=next(f for f in x['filenames'] if f.endswith('.rlib'))
assert len(libs)==5
helper=fixed.parent/'metadata-compat'; cmd=['rustc','--edition=2021','-O',str(D/'arena-compat.rs'),'-L','dependency='+str(R/'target/release/deps')]
for n,p in libs.items():cmd+=['--extern',n+'='+p]
cmd+=['-o',str(helper)]; run('metadata-compat-compile',cmd)
old=R/'docs/evidence/m0-t13a-model-workspace'; baseline=json.loads((old/'compat-proof.json').read_text())
manifest=json.loads((old/'legacy-input-manifest.json').read_text())
def check_old():
    for rel in [baseline['source_original'],baseline['copy_complete_retained_at']]:
        root=R/rel;actual={p.relative_to(root).as_posix():p for p in root.rglob('*') if p.is_file()}
        assert set(actual)==set(manifest)
        for n,p in actual.items():assert p.stat().st_size==manifest[n]['bytes'] and sha(p)==manifest[n]['sha256'],str(p)
check_old()
run('metadata-compat-run',[str(helper),str(R/baseline['copy_complete_retained_at']),baseline['rules_identity_unchanged'],str(R/'work/t13a65-joint-02/model/business/fault-reference/cpu-final-engine.json')])
check_old()
save('metadata-compat-proof.json',dict(result=json.loads((D/'metadata-compat-run.stdout').read_text()),
    source_sha256=source,helper_sha256=sha(helper),helper_source_sha256=sha(D/'arena-compat.rs'),
    original_and_copy_files_verified_unchanged=243,linked_libraries={n:dict(path=p,sha256=sha(p)) for n,p in libs.items()}))

v1=R/'work/t13a-role-metadata-v1-proof'
run('arena-current-reader-v1',[str(helper),str(v1/'world'),'metadata-compat-proof-v1',str(v1/'expected-engine.json')])
world=R/'work/t13a-role-arena-business-01';assert not world.exists()
env=os.environ.copy()
for key in ['MYCITY_NORMAL_POOL','MYCITY_BROKER_SESSION','MYCITY_BROKER_SUPERVISOR','MYCITY_NORMAL_WORKER_GRANT','MYCITY_ROLE_CONSTRUCTION_OOM']:env.pop(key,None)
run('arena-full-business',[str(fixed),'speed-run',str(world),'unpaced'],env)
report=json.loads((world/'report.json').read_text())
business=json.loads((world/'business/report.json').read_text())
assert report['experiment_completed'] and report['full_M0']=='not_accepted'
assert business['completed_hours']==720 and business['verified_targets']==723 and business['worker_windows']==724
assert business['entire_engine_including_metadata']=='equal_independent_normal_reference'
save('arena-full-business-report.json',dict(supervisor=report,business=business))
save('arena-terminal.json',dict(source_sha256=source,functional_original_full_30day_verified=True,
 l1_pressure_included=False,formal_performance_accepted=False,eight_hour_accepted=False,memory_16gb_gate='not_run',full_m0='not_accepted'))
print('Frozen arena source regression, old complete world and actual brokered full30d diagnostic terminal0',flush=True)
