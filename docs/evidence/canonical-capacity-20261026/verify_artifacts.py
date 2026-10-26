"""Read-only artifact identity check; does not rerun the game or prove acceptance."""
import hashlib,json,pathlib,zipfile
root=pathlib.Path(__file__).parent
def sha(b):return hashlib.sha256(b).hexdigest()
for item in json.loads((root/'retained-originals.json').read_text())['files']:
 p=pathlib.Path(item['file'])
 assert p.is_file() and p.stat().st_size==item['bytes'] and sha(p.read_bytes())==item['sha256'],p
for item in json.loads((root/'verified-programs.json').read_text()):
 p=pathlib.Path(item['file'])
 if 'member' in item:
  assert sha(p.read_bytes())==item['archive_sha256'],p
  with zipfile.ZipFile(p) as z:b=z.read(item['member'])
 else:b=p.read_bytes()
 assert sha(b)==item['sha256'],p
print('Full retained files and executable/archive members match; identity only, no new game run')
