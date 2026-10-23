"""Literal checks of complete retained checkpoint and exact candidate artifacts; not replay."""
from pathlib import Path
import hashlib
import json
import sys

world, candidate, fixture = map(Path, sys.argv[1:])
checkpoints = []
for path in world.glob('checkpoint-*.json'):
    value = json.loads(path.read_bytes())
    if 'engine' in value:
        checkpoints.append((value['sequence'], path, value))
sequence, path, checkpoint = max(checkpoints, key=lambda row: row[0])
engine = json.loads(bytes(checkpoint['engine']))
cash = {row['value']['id']: row['value']['cash']
        for row in engine['world']['accounts']['slots'] if row['value'] is not None}
register = engine['world']['town']['state']['equity_registers'][0]
claim = next(c for c in engine['timeline']['claims'] if c['id'] == 100)
scene = json.loads((candidate/'candidate-scene-input.json').read_bytes())
profile_bytes = (candidate/'candidate-profile.json').read_bytes()
profile = json.loads(profile_bytes)
report = json.loads((candidate/'candidate-report.json').read_bytes())
parent = json.loads(fixture.read_bytes())['scene']
checks = {
    'actual_cash': cash == {1: 1000076, 2: 799924, 3: 0, 4: 1200000},
    'conserved_cash': sum(cash.values()) == 3000000,
    'same_loan_paid_once': all(claim[k] == v for k,v in {
        'payer': 3, 'payee': 4, 'due': 5*86400, 'principal': 1000,
        'paid': 1000, 'processed': True, 'source_ref': 2}.items()),
    'capital_and_surplus': register['cumulative_capital_returned'] == 620100
        and register['cumulative_liquidation_surplus_distributed'] == 45,
    'company_closed': engine['world']['town']['state']['exit']['phase'] == 'closed',
    'door_literal': scene['door']['point'] == {'x': 4000, 'y': 3000}
        and scene['door']['facing'] == 'east',
    'rights_inherited': all(scene['door'][k] == parent['door'][k]
        for k in ['z_cm', 'mode', 'clear_width_cm', 'can_enter', 'can_exit', 'authorized']),
    'same_road_anchor': scene['road'] == parent['road'] and scene['anchor'] == parent['anchor'],
    'geometry_revision': scene['geometry_revision'] == parent['geometry_revision'] + 1,
    'candidate_payload_hash': hashlib.sha256(profile_bytes).hexdigest()
        == report['source_sha256']['candidate_profile'],
    'fixture_identity': hashlib.sha256(fixture.read_bytes()).hexdigest()
        == report['source_sha256']['spatial_fixture_bytes'],
    'no_activation_claim': report['result'] == 'candidate_only_not_activated'
        and report['persistence'] == 'not_written_to_store',
    'fixture_has_no_traffic_delta': report['movement']['candidate_served_people_per_period'] == 4
        and report['movement']['prior_edge_free_flow_seconds']
            == report['movement']['candidate_edge_free_flow_seconds'],
}
assert all(checks.values()), checks
print(json.dumps({'passed': True, 'scope': 'literal_checkpoint_and_candidate_artifacts_not_replay',
    'world': str(world), 'candidate': str(candidate), 'sequence': sequence,
    'checkpoint_sha256': hashlib.sha256(path.read_bytes()).hexdigest(),
    'checks': checks, 'cash': cash}, indent=2))
