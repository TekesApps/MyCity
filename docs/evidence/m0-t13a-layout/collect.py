#!/usr/bin/env python3
"""Archive bounded reports/logs and optionally hash immutable layout snapshots.
Usage: collect.py RUN_DIR PREFIX [--payloads]
No payload is deleted or changed. Large pressure bytes stay in ignored work/.
"""
import hashlib,json,shutil,sys,zipfile
from pathlib import Path
root=Path(__file__).resolve().parents[3]
run=Path(sys.argv[1]); prefix=sys.argv[2]; out=Path(__file__).parent

def sha(p):
    h=hashlib.sha256()
    with p.open('rb') as f:
        while b:=f.read(1024*1024):h.update(b)
    return h.hexdigest()
for src,name in [(run/'model/report.json','model.json'),(run/'observer-report.json','observer.json')]:
    if src.exists():shutil.copyfile(src,out/f'{prefix}-{name}')
with zipfile.ZipFile(out/f'{prefix}-measurements.zip','w',zipfile.ZIP_DEFLATED) as z:
    for name in ['host-samples.jsonl','model.stdout','model.stderr','model/events.jsonl','model/compute-durations.bin','model/manifest.json']:
        p=run/name
        if p.exists():z.write(p,name)
if '--payloads' in sys.argv:
    report=[]
    for d in sorted((run/'model/snapshots').glob('snapshot-*')):
        index=json.loads((d/'index.json').read_text()); files=[]
        shutil.copyfile(d/'index.json',out/f'{prefix}-{d.name}-index.json')
        for p in sorted(d.iterdir()):
            if p.is_file():files.append({'name':p.name,'bytes':p.stat().st_size,'sha256':sha(p)})
        report.append({'snapshot':d.name,'index':index,'actual_files':files})
    (out/f'{prefix}-payload-audit.json').write_text(json.dumps(report,indent=2)+'\n')
# Capture the exact source-hash preimage used by build.rs, not a stale Git HEAD.
paths=['.cargo/config.toml','Cargo.toml','Cargo.lock','rust-toolchain.toml']
for folder in ['crates','fixtures']:paths.extend(str(p.relative_to(root)) for p in (root/folder).rglob('*') if p.is_file())
h=hashlib.sha256()
with zipfile.ZipFile(out/f'{prefix}-source.zip','w',zipfile.ZIP_DEFLATED) as z:
    for name in sorted(paths):
        b=(root/name).read_bytes(); n=name.encode();h.update(len(n).to_bytes(8,'little'));h.update(n);h.update(len(b).to_bytes(8,'little'));h.update(b);z.writestr(name,b)
(out/f'{prefix}-source.json').write_text(json.dumps({'source_sha256':h.hexdigest(),'source_files':len(paths),'capsule_sha256':sha(out/f'{prefix}-source.zip')},indent=2)+'\n')
print(prefix,h.hexdigest())
