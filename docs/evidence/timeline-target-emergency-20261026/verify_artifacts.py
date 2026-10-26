"""Read-only identity checks; does not execute the game or replace acceptance."""
from pathlib import Path
import hashlib,json
root=Path(__file__).resolve().parents[3];e=Path(__file__).resolve().parent
for row in json.loads((e/'source02-manifest.json').read_text()):
 assert hashlib.sha256((root/row['file']).read_bytes()).hexdigest()==row['sha256'],row['file']
index=json.loads((e/'retained-originals08.json').read_text())
for row in index['files']:
 p=Path(row['path']);assert p.is_file(),row['path']
 assert p.stat().st_size==row['bytes'] and hashlib.sha256(p.read_bytes()).hexdigest()==row['sha256'],row['path']
assert index['logical_original_bytes']<index['retention_cap_bytes']
print('current source and full indexed local originals match; identity only, not additional game testing')
