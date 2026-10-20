#!/usr/bin/env python3
"""Verify this frozen source/evidence, optionally rerun the actual process experiment."""
import argparse
import hashlib
import json
from pathlib import Path
import struct
import subprocess
import tempfile
import zipfile

HERE = Path(__file__).resolve().parent
ROOT = HERE.parents[2]
def check(condition, message):
    if not condition:
        raise SystemExit(message)
def hash_bytes(data):
    return hashlib.sha256(data).hexdigest()
def source_hash():
    names = ['.cargo/config.toml', 'Cargo.toml', 'Cargo.lock', 'rust-toolchain.toml']
    names += [p.relative_to(ROOT).as_posix() for folder in ['crates', 'fixtures'] for p in (ROOT / folder).rglob('*') if p.is_file()]
    h = hashlib.sha256()
    for name in sorted(names):
        name_bytes = name.encode()
        data = (ROOT / name).read_bytes()
        h.update(struct.pack('<Q', len(name_bytes)))
        h.update(name_bytes)
        h.update(struct.pack('<Q', len(data)))
        h.update(data)
    return h.hexdigest()
def check_report(report):
    check(report['source_sha256'] == source_hash(), 'frozen source differs')
    check(report['two_jobs_same_process'] and report['fault_stage_barrier'], 'missing persistent/gated experiment')
    check(report['continuous_report']['time'] == 180, 'missing continuous window')
    check(report['continuous_report']['emitted_micro'] == 600000, 'emissions reset or duplicated')
    check({f['case'] for f in report['faults']} == {'source_kill','tile_kill','published_kill'}, 'missing fault case')
    for f in report['faults']:
        check(f['termination'] == 'signal' and f['killed_pid'] != f['recovered_pid'], 'no independent killed/restarted child')
        check(f['recomputed_exact_cpu_state'], 'CPU comparison missing')
        if f['case'] != 'published_kill':
            limit = 20000 if f['case'] == 'source_kill' else 16384
            check(0 < f['partial_cursor'] < limit, 'kill did not hold a partial page')
    check(report['world_store_integration'] == 'not_implemented', 'scope expanded')
    check(report['memory_16gb_gate'] == 'not_run', '16GB claimed')
def main():
    parser = argparse.ArgumentParser()
    parser.add_argument('--rerun', action='store_true')
    args = parser.parse_args()
    report = json.loads((HERE / 'probe.json').read_bytes())
    check_report(report)
    gates = json.loads((HERE / 'gates.json').read_bytes())
    check({g['gate'] for g in gates} == {'fmt','release-tests','clippy'} and all(g['exit_code'] == 0 for g in gates), 'incomplete release gates')
    check('test result: FAILED' not in (HERE / 'release-tests.txt').read_text(), 'failed release tests')
    with zipfile.ZipFile(HERE / 'tasks.zip') as archive:
        check(len(archive.namelist()) <= 64, 'archive exceeds finite case count')
        assets = {}
        for entry in archive.infolist():
            check(entry.file_size <= 8 * 1024 * 1024, 'oversized artifact')
            data = archive.read(entry)
            name = Path(entry.filename)
            if name.suffix == '.air':
                check(name.stem == hash_bytes(data), 'asset hash mismatch')
                cp = json.loads(data)
                payload = json.dumps(cp['payload'], separators=(',', ':'), ensure_ascii=False).encode()
                check(cp['sha256'] == hash_bytes(payload), 'checkpoint payload mismatch')
                assets[entry.filename] = cp['payload']
            elif name.name == 'events.json':
                events = json.loads(data)
                check(all(e['protocol'] == report['protocol'] and e['ticket']['source_sha256'] == report['source_sha256'] for e in events), 'event identity mismatch')
                if name.parent.name != 'persistent':
                    check(not any(e['stage'] == 'complete' and e['ticket']['generation'] == 1 for e in events), 'killed generation completed')
                    check(any(e['stage'] == 'complete' and e['ticket']['generation'] == 2 for e in events), 'retry completion absent')
        for f in report['faults']:
            state = assets[f"{f['case']}/candidates/{f['candidate_sha256']}.air"]
            for field, width, report_key in [('field', 'I', 'field_sha256'),('integral', 'Q', 'integral_sha256')]:
                bits = b''.join(struct.pack('<' + width, n) for n in state[field])
                check(hash_bytes(bits) == f['report'][report_key], 'reported field/integral mismatch')
            check(state['time'] == state['input']['target'] and state['progress'] is None, 'partial candidate')
    if args.rerun:
        subprocess.run(['cargo','build','-p','mycity-headless','--release','--locked','--offline'], cwd=ROOT, check=True)
        with tempfile.TemporaryDirectory(prefix='mycity-worker-rerun-') as temporary:
            output = subprocess.check_output([str(ROOT / 'target/release/mycity-headless'), 'worker-probe', str(Path(temporary) / 'tasks')], cwd=ROOT)
            check_report(json.loads(output))
    print('W1 evidence verified; W2-W4 and actual 16GB gate remain unaccepted.')
if __name__ == '__main__':
    main()
