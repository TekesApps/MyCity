#!/usr/bin/env python3
"""Create a NEW read-only fixture view with one wrong declared native hash.
Immutable payloads are hard-linked, never edited. Original snapshot is untouched.
"""
import hashlib,json,os,sys
from pathlib import Path
if len(sys.argv)!=3:raise SystemExit('negative-fixture.py ORIGINAL_SNAPSHOT NEW_FIXTURE_ROOT')
source=Path(sys.argv[1]).resolve(strict=True);root=Path(sys.argv[2]);root.mkdir();point=root/'snapshot';point.mkdir()
original=(source/'index.json').read_bytes();index=json.loads(original)
for entry in index['files']:
    name=entry['name']
    if '/' in name or name.startswith('.'):raise SystemExit('unsafe source member')
    p=source/name
    if p.is_symlink() or not p.is_file():raise SystemExit('not regular immutable payload')
    os.link(p,point/name)
    if name=='native.json':entry['sha256']='0'*64
(point/'index.json').write_text(json.dumps(index,indent=2)+'\n')
(root/'negative-input.json').write_text(json.dumps({'source':str(source),'original_index_sha256':hashlib.sha256(original).hexdigest(),'source_bytes_unchanged':(source/'index.json').read_bytes()==original,'mutated_field':'files[native.json].sha256','expected_error':'layout_restore_checksum:native.json','payloads_are_immutable_hardlinks':True,'financial_world':False},indent=2)+'\n')
print(point)
