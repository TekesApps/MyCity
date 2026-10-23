"""Read-only literal checks for the retained paid-wage CLI checkpoint, not WAL replay."""
import hashlib
import json
import sys
from pathlib import Path

world = Path(sys.argv[1])
rows = []
for path in world.glob('checkpoint-*.json'):
    value = json.loads(path.read_bytes())
    if 'engine' in value:
        rows.append((value['sequence'], path, value))
sequence, path, checkpoint = max(rows, key=lambda row: row[0])
engine = json.loads(bytes(checkpoint['engine']))
window = engine['world']['housing']['paid_wages']
payroll = engine['timeline']['payroll']
cash = {row['value']['id']: row['value']['cash']
        for row in engine['world']['accounts']['slots'] if row['value'] is not None}
checks = {
    'completed_day32': engine['timeline']['progress']['last_complete'] == 32*86400,
    'one_actual_monthly_payment': len(payroll) == 1 and payroll[0]['paid'] == 198400
        and payroll[0]['billed'] == 198400 and payroll[0]['accrued'] == 204800,
    'observed_since_day0': window['coverage_start_day_plus_one'] == 1,
    'ring_shape': len(window['days']) == 32
        and all(len(day['amounts']) == 32 for day in window['days']),
    'only_day31_received_cash': [(d['day_plus_one'], d['amounts'][0])
        for d in window['days'] if any(d['amounts'])] == [(32, 198400)],
    'no_other_household_payment': all(not any(d['amounts'][1:]) for d in window['days']),
    'actual_accounts': cash == {1: 1000000, 2: 200000+198400, 3: 600000-198400, 4: 1200000},
    'conserved_total_cash': sum(cash.values()) == 3000000,
    'unoccupied_fixture_is_not_move': engine['world']['housing']['state']['units'][0]['household_id'] == 0,
    'no_rent_claim_in_income_only_fixture': engine['timeline']['claims'] == [],
}
assert all(checks.values()), checks
print(json.dumps({'passed': True, 'scope': 'literal_paid_wage_checkpoint_not_WAL_replay',
    'world': str(world), 'checkpoint': path.name, 'sequence': sequence,
    'checkpoint_sha256': hashlib.sha256(path.read_bytes()).hexdigest(),
    'checks': checks, 'cash': cash}, indent=2))
