#!/usr/bin/env python3
"""Archive a completed formal W4 run; never package a dry/live run as accepted."""
import argparse
import hashlib
import json
import sys
import zipfile
from pathlib import Path

sys.dont_write_bytecode = True
import verify


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--run', required=True, type=Path)
    args = parser.parse_args()
    root = args.run.resolve(strict=True)
    launch = json.loads((verify.HERE / 'launch.json').read_bytes())
    verify.require(launch.get('exit_code') == 0 and launch['command_wall_seconds'] >= 1800,
                   'formal process has not successfully exited')
    verify.require(Path(launch['command'][2]).resolve() == root and launch['command'][3] == '1800',
                   'launch command/run mismatch')
    verify.require(verify.source_hash() == verify.SOURCE == launch['source_sha256'], 'frozen source differs')
    verify.require(hashlib.sha256(Path(launch['command'][0]).read_bytes()).hexdigest() == launch['binary_sha256'],
                   'executable replaced during experiment')
    checks = verify.check(lambda name: (root / name).read_bytes(), False)
    files = sorted(p for p in root.rglob('*') if p.is_file())
    verify.require(len(files) <= 2048 and sum(p.stat().st_size for p in files) <= 192 * 1024**2, 'artifact cap')
    verify.require(all(not p.is_symlink() for p in root.rglob('*')), 'symlink in artifacts')
    temporary = verify.HERE / 'run.zip.pending'
    manifest = {}
    with zipfile.ZipFile(temporary, 'w', compression=zipfile.ZIP_DEFLATED, compresslevel=9) as archive:
        for path in files:
            name, raw = path.relative_to(root).as_posix(), path.read_bytes()
            manifest[name] = {'bytes': len(raw), 'sha256': verify.sha(raw)}
            archive.writestr(name, raw)
    temporary.replace(verify.HERE / 'run.zip')
    (verify.HERE / 'artifact-manifest.json').write_text(json.dumps(manifest, indent=2, sort_keys=True) + '\n')
    (verify.HERE / 'probe.json').write_bytes((root / 'report.json').read_bytes())
    print(json.dumps({'files': len(files), 'compressed_bytes': (verify.HERE / 'run.zip').stat().st_size,
                      'uncompressed_bytes': sum(v['bytes'] for v in manifest.values()), 'checks': checks}, indent=2))


if __name__ == '__main__':
    main()
