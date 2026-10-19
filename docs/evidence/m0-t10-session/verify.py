"""Verify local frozen-source evidence; no world mutation or performance extrapolation."""
from pathlib import Path
import hashlib
import json
import re

ROOT = Path(__file__).resolve().parents[3]
EVIDENCE = Path(__file__).resolve().parent
paths = ['.cargo/config.toml', 'Cargo.toml', 'Cargo.lock', 'rust-toolchain.toml']
for directory in ['crates', 'fixtures']:
    paths += [str(p.relative_to(ROOT)) for p in (ROOT / directory).rglob('*') if p.is_file()]
digest = hashlib.sha256()
for name in sorted(paths):
    content = (ROOT / name).read_bytes()
    encoded = name.encode()
    digest.update(len(encoded).to_bytes(8, 'little'))
    digest.update(encoded)
    digest.update(len(content).to_bytes(8, 'little'))
    digest.update(content)
provenance = json.loads((EVIDENCE / 'provenance.json').read_text())
log = (EVIDENCE / 'tests.txt').read_text()
rows = re.findall(r'test result: ok\. (\d+) passed; (\d+) failed; (\d+) ignored;', log)
assert rows and all(int(failed) == 0 and int(ignored) == 0 for _, failed, ignored in rows)
passed = sum(int(count) for count, _, _ in rows)
assert passed == 167, passed
matches = re.findall(r'T10_PACED_DAY (\{[^\n]+\})', log)
assert len(matches) == 1
paced = json.loads(matches[0])
assert digest.hexdigest() == provenance['source_sha256'] == paced['source_sha256']
assert hashlib.sha256((ROOT / 'Cargo.lock').read_bytes()).hexdigest() == provenance['lock_sha256']
assert paced['same_state_as_unpaced'] and paced['persistent_commits'] == 24
assert paced['game_seconds'] == 86400 and paced['actual_wall_seconds'] >= 60
assert paced['target_exactly_met'] == (paced['actual_wall_seconds'] <= 60)
assert 'error:' not in (EVIDENCE / 'clippy.txt').read_text()
assert 'error:' not in (EVIDENCE / 'fmt.txt').read_text()
summary = {
    'source_sha256': digest.hexdigest(), 'release_tests_passed': passed,
    'functional_t10a_gate': 'passed', 'paced_economy_day': paced,
    'strict_economy_60_second_gate': 'passed' if paced['target_exactly_met'] else 'not_met',
    'full_t10': 'not_completed', 'full_source_scheduler': 'not_implemented',
    'memory_16gb_gate': 'not_run',
    'tests_sha256': hashlib.sha256((EVIDENCE / 'tests.txt').read_bytes()).hexdigest(),
}
(EVIDENCE / 'verified-summary.json').write_text(json.dumps(summary, indent=2) + '\n')
print(json.dumps(summary, indent=2))
