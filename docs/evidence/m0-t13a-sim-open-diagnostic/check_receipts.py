"""Verify bounded persisted Sim diagnostics; no native-memory/performance verdict."""
import argparse
import hashlib
import json
from pathlib import Path

p = argparse.ArgumentParser()
p.add_argument('world', type=Path)
p.add_argument('pin', type=Path)
a = p.parse_args()
report = json.loads((a.world / 'report.json').read_text())
pin = json.loads(a.pin.read_text())
assert report['source_sha256'] == pin['source_sha256']
assert hashlib.sha256(Path(pin['binary']).read_bytes()).hexdigest() == pin['binary_sha256']
assert report['tier'] == 'full'
assert report['completed_hour_targets'] == 720
assert report['verified_drive_targets'] == 723
assert report['attempt_failures'] == 0
assert report['complete_final_business_and_air'] == 'passed'
assert report['cold_complete_image_unchanged'] is True
assert report['oldest_receipt_retry'] == 'passed_without_side_effect'
for field in ('eight_hour_gate', 'memory_16gb_gate', 'performance_acceptance'):
    assert report[field] == 'not_run'
assert report['full_M0'] == 'not_accepted'
receipts = []
seen = set()
for line in (a.world / 'transcript.jsonl').read_bytes().splitlines():
    record = json.loads(line)
    receipt = record.get('data', {}).get('sim_open_memory_diagnostic')
    if receipt is None:
        continue
    assert len(line) <= 4096
    assert receipt['schema'] == 'mycity-sim-open-diagnostic-receipt-v1'
    identity = (receipt['sim_pid'], receipt['sim_birth_unix_ns'])
    assert identity not in seen
    seen.add(identity)
    clocks = []
    for phase in ('pre_open', 'post_open'):
        item = receipt[phase]
        for edge in ('clock_begin', 'clock_end'):
            clock = item[edge]
            assert (clock['pid'], clock['instance_unix_ns']) == identity
            clocks.append(clock['elapsed_ns'])
        arena = item['role_arena']
        assert arena['allocation_failures'] == 0
        assert arena['arena_used_bytes'] + arena['arena_free_bytes'] == arena['arena_usable_bytes']
        assert arena['activation']['pid'] == identity[0]
    event = receipt['event_clock']
    assert (event['pid'], event['instance_unix_ns']) == identity
    clocks.append(event['elapsed_ns'])
    assert clocks == sorted(clocks)
    workspace = receipt['model_workspace']
    assert workspace['active_slot']['owned_model_bytes'] > 0
    assert workspace['spare_restore_slot']['owned_model_bytes'] > 0
    assert workspace['codec_backing_bytes'] > 0
    assert receipt['store_owned_bytes'] >= workspace['workspace_owned_bytes']
    receipts.append({'identity': identity, 'operation': receipt['operation'],
                     'clock_brackets_ns': clocks, 'store_owned_bytes': receipt['store_owned_bytes'],
                     'workspace_owned_bytes_included_in_store': workspace['workspace_owned_bytes'],
                     'serialized_line_bytes': len(line)})
assert 1 <= len(receipts) <= 8
assert [r['operation'] for r in receipts] == ['create', 'open']
print(json.dumps({'functional_receipts': 'passed', 'source_sha256': pin['source_sha256'],
                  'receipts': receipts, 'native_peak_or_birth_alignment': 'not_measured',
                  'performance_acceptance': 'not_run', 'storage': 'external HP P900 USB/APFS SSD'}, indent=2))
