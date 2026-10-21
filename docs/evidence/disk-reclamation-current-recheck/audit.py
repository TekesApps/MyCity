"""Read-only one-shot inventory; never deletes or rebuilds anything."""
import gzip
import json
import os
import shutil
import stat
import subprocess
from pathlib import Path

R = Path('/Users/frankqin/dev/MyCity')
D = Path(__file__).resolve().parent
assert not (D / 'result.json').exists(), 'one-shot record already exists'

def inventory():
    rows = []
    for root in ('work', 'target'):
        for parent, dirs, files in os.walk(R / root, followlinks=False):
            for name in sorted(files):
                p = Path(parent) / name
                s = p.lstat()
                rows.append([str(p.relative_to(R)), s.st_dev, s.st_ino,
                             s.st_mode, s.st_size, s.st_mtime_ns,
                             s.st_blocks, s.st_nlink])
    return sorted(rows)

def observations(label):
    for name, command in (
        ('processes', ['ps', '-axo', 'pid,ppid,etime,command']),
        ('open-files', ['lsof', '-nP', '+D', str(R / 'work'), '+D', str(R / 'target')]),
    ):
        result = subprocess.run(command, text=True, capture_output=True)
        output = result.stdout
        if name == 'processes':
            lines = output.splitlines()
            selected = lines[:1]
            for line in lines[1:]:
                fields = line.split(None, 3)
                if len(fields) == 4:
                    executable = fields[3].split()[0]
                    if any(x in executable.lower() for x in ('mycity', 'cargo', 'rustc')):
                        selected.append(' '.join(fields[:3] + [executable]))
            output = '\n'.join(selected) + '\n'
        (D / f'{label}-{name}.txt').write_text(output)
        (D / f'{label}-{name}.stderr').write_text(result.stderr)
        if name == 'open-files':
            assert result.returncode == 1 and not result.stdout and not result.stderr
    return {
        'du_kib': {name: int(subprocess.check_output(['du', '-sk', str(R / name)], text=True).split()[0])
                   for name in ('work', 'target')},
        'filesystem_available_bytes': shutil.disk_usage(R).free,
    }

before = observations('before')
rows = inventory()
with gzip.open(D / 'inventory-before.json.gz', 'wt') as f:
    json.dump(rows, f, separators=(',', ':'))
payloads = [r for r in rows if Path(r[0]).name in ('pressure.bin', 'work-reserve.bin')]
assert len(payloads) == 12, 'reassess newly created or missing pressure inputs'
(D / 'retained-payloads.json').write_text(json.dumps(payloads, indent=2) + '\n')
after_rows = inventory()
assert rows == after_rows, 'concurrent mutation: do not claim unchanged'
after = observations('after')
result = {
    'mode': 'read_only_no_build_no_delete', 'before': before, 'after': after,
    'inventory_entries': len(rows), 'inventory_unchanged': True,
    'retained_pressure_files': len(payloads),
    'retained_pressure_allocated_bytes': sum(r[6] * 512 for r in payloads),
    'deleted': [], 'deleted_inode_allocated_bytes': 0,
    'filesystem_available_delta_bytes': after['filesystem_available_bytes'] - before['filesystem_available_bytes'],
    'attribution': 'df delta is observational only; no bytes credited to this audit',
    'source_and_wip': 'not modified by audit; no cleanup command executed',
}
(D / 'deleted.json').write_text('[]\n')
(D / 'result.json').write_text(json.dumps(result, indent=2) + '\n')
print(json.dumps(result, indent=2))
