"""Bounded decoder of pre-operation evidence, never an OOM-instant claim."""
import json
from pathlib import Path
import struct
import sys

FIELDS = ('magic', 'sequence', 'generation', 'phase', 'index', 'request_bytes',
          'requested', 'arena_used', 'arena_free', 'allocation_failures',
          'reserve_available_pages', 'total_loan_pages', 'decode_loan_pages',
          'elapsed_ns', 'reserved_0', 'reserved_1')
PHASES = {1: 'write_file', 2: 'decode_loan', 3: 'native_decode',
          4: 'native_roundtrip', 5: 'table_decode', 6: 'pressure_compare',
          7: 'reserve_compare', 8: 'decode_repay_before_page',
          9: 'decode_repaid_before_publication'}

def decode(path):
    with Path(path).open('rb') as f:
        raw = f.read(128 * 256 + 1)
    if not raw or len(raw) > 128 * 256 or len(raw) % 128:
        raise ValueError('trace_shape_or_truncated_record')
    rows = [dict(zip(FIELDS, struct.unpack('<16Q', raw[i:i+128])))
            for i in range(0, len(raw), 128)]
    for i, r in enumerate(rows):
        if r['magic'] != 0x4d59435452414301 or r['sequence'] != i:
            raise ValueError('trace_identity_or_sequence')
        if r['generation'] != rows[0]['generation'] or r['phase'] not in PHASES:
            raise ValueError('trace_generation_or_phase')
        if r['decode_loan_pages'] > r['total_loan_pages'] or r['reserved_0'] or r['reserved_1']:
            raise ValueError('trace_fields')
        r['phase_name'] = PHASES[r['phase']]
    return dict(path=str(path), records=len(rows), last_pre_operation=rows[-1],
                maximum_pre_operation_requested=max(r['requested'] for r in rows),
                minimum_pre_operation_free=min(r['arena_free'] for r in rows),
                final_decode_repayment_observed=rows[-1]['phase'] == 9 and rows[-1]['decode_loan_pages'] == 0,
                semantics='sample before operation; not exact failing allocation or contiguous-free proof; phase 9 is before index publication and COW return',
                rows=rows)

if __name__ == '__main__':
    print(json.dumps(decode(sys.argv[1]), indent=2))
