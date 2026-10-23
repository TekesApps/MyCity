from pathlib import Path
import json,subprocess,sys
r=Path(__file__).resolve().parent;m=json.loads((r/'integer-integrated-headless.json').read_text())
p=subprocess.run([m['binary'],'baseline','fixtures/m0-bootstrap.json'],capture_output=True,text=True)
print(p.stdout,end='');print(p.stderr,end='',file=sys.stderr)
if p.returncode:sys.exit(p.returncode)
a=json.loads(p.stdout);assert a['source_sha256']==m['source_sha256']
(r/'112-canonical-integrated-baseline-corrected'/'metadata-source-check.json').write_text(json.dumps({'matched':True,'embedded_source_sha256':a['source_sha256'],'binary_sha256':m['binary_sha256']},indent=2))
