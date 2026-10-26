"""Verify frozen source and the complete unintegrated-WIP review supplement. No tests run."""
from pathlib import Path
import hashlib,json,zipfile
root=Path(__file__).resolve().parents[3]
e=Path(__file__).resolve().parent
source=json.loads((e/'active-source-manifest.json').read_text())
for row in source:
    f=root/row['file']
    assert hashlib.sha256(f.read_bytes()).hexdigest()==row['sha256'],row['file']
w=json.loads((e/'unintegrated-wip-manifest.json').read_text())
assert hashlib.sha256((e/'unintegrated-wip.zip').read_bytes()).hexdigest()==w['archive_sha256']
with zipfile.ZipFile(e/'unintegrated-wip.zip') as z:
    assert set(z.namelist())==set(w['files'])
    for name,row in w['files'].items():
        assert hashlib.sha256(z.read(name)).hexdigest()==row['sha256'],name
print(f"source files={len(source)}; full WIP originals={len(w['files'])}; artifact identity only, not game acceptance")
