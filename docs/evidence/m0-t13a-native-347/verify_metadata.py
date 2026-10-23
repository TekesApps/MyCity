"""Reproduce metadata/arithmetic checks; full payload replay is the pinned Rust CLI."""
import json
from pathlib import Path

root = Path(__file__).resolve().parent
index = json.loads((root / 'original-snapshot-index.json').read_text())
report = json.loads((root / 'cold-recovery-report.json').read_text())
pin = json.loads((root / 'reader-fixed-binary.json').read_text())
assert report['restore']['reader_source_sha256'] == pin['source_sha256']
assert report['restore']['same_source'] is False
assert report['restore']['all_payload_files_hashed_and_loaded'] is True
assert report['cold_reopen_and_scan'] == 'passed'
assert len(index['files']) == 29
assert sum(row['bytes'] for row in index['files']) == report['restore']['byte_length'] == 3372538130
assert report['restore']['restored_generation'] == 2
assert report['restore']['restored_mutation_iteration'] == 300
assert report['resumed_scan']['iteration'] == 301
native = report['manifest']['native']
assert (native['account_arena']['record_bytes'], native['account_arena']['slot_bytes'], native['account_arena']['slots_capacity']) == (32, 40, 40000)
assert (native['inventory_arena']['record_bytes'], native['inventory_arena']['slot_bytes'], native['inventory_arena']['slots_capacity']) == (96, 104, 110000)
expected_vectors = [['account_index', 40000, 40000, 32], ['inventory_index', 110000, 110000, 32], ['claims', 180000, 180000, 48], ['payroll', 31200, 31200, 120], ['resources', 8, 8, 16], ['leases', 64, 64, 24]]
assert native['vectors'] == expected_vectors
owned = 304 + 40000 * (40 + 4) + 110000 * (104 + 4) + sum(cap * stride for _, _, cap, stride in expected_vectors)
assert owned == 30825968 == native['owned_capacity_bytes']
workspace = report['manifest']['decode_workspace']
assert workspace['inline_header_bytes'] == 464
assert workspace['native_inline_header_bytes'] == 304
assert sum(t['owned_bytes'] for t in workspace['table_backings']) == 32144000
assert workspace['initialized_tail_bytes'] == 4138736
assert 464 + owned - 304 + 32144000 + 4138736 == 67108864 == workspace['owned_bytes']
assert report['manifest']['cow_reserve_bytes'] == 134217728
assert len(native['fields']) == 16
print('passed: source/byte totals, pinned ABI/capacities and fixed workspace arithmetic; no independent payload replay')
