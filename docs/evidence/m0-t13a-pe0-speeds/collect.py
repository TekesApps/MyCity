#!/usr/bin/env python3
"""Archive terminal diagnostic metadata and real business; large L1 pressure stays in work."""
import hashlib,json,struct,sys,zipfile
from pathlib import Path
D=Path(__file__).resolve().parent;R=D.parents[2];base=R/'work'/sys.argv[1];out=D/sys.argv[1];out.mkdir()
manifest=[];omitted=[]
with zipfile.ZipFile(out/'artifacts.zip','x',zipfile.ZIP_DEFLATED) as z:
 for p in sorted(base.rglob('*')):
  assert not p.is_symlink()
  if not p.is_file():continue
  n=p.relative_to(base).as_posix()
  if '/layout/snapshots/snapshot-' in n and p.name!='index.json':
   h=hashlib.sha256()
   with p.open('rb') as f:
    while b:=f.read(65536):h.update(b)
   omitted.append({'path':n,'bytes':p.stat().st_size,'sha256':h.hexdigest(),'integrity':'actual bytes hashed after terminal; retained in ignored work; pending files are NOT complete restored snapshots'});continue
  b=p.read_bytes();manifest.append({'path':n,'bytes':len(b),'sha256':hashlib.sha256(b).hexdigest()});z.writestr(n,b)
(out/'manifest.json').write_text(json.dumps(manifest,indent=2)+'\n');(out/'large-files-retained.json').write_text(json.dumps(omitted,indent=2)+'\n')
