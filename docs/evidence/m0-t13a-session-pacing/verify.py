#!/usr/bin/env python3
"""Audit actual session regression logs; never extrapolate one day to 30 days."""
import json,re
from pathlib import Path
D=Path(__file__).resolve().parent
info=json.loads((D/'source.json').read_text())
s=(D/'session-tests.txt').read_text()
assert 'test result: ok. 1 passed; 0 failed; 0 ignored;' in s
assert 'test result: ok. 15 passed; 0 failed; 0 ignored;' in s
for name in ['computation_consumes_interval_but_idle_time_does_not_create_credit','replay_conflict_and_rejected_requests_cannot_reset_release','paced_economy_day_matches_unpaced_committed_business_and_reports_real_wall_time','pause_step_run_and_cold_receipt_are_exact_across_real_reopen','disk_backpressure_cannot_be_bypassed_by_step_or_speed','huge_time_speed_and_stale_cursor_reject_without_mutation']:
 assert f'{name} ... ok' in s
r=[json.loads(line.split(' ',1)[1]) for line in s.splitlines() if line.startswith('T10_PACED_DAY ')];assert len(r)==1;r=r[0]
assert r['source_sha256']==info['source_sha256'] and r['game_seconds']==86400 and r['persistent_commits']==24
assert r['same_state_as_unpaced'] and r['state_sha256']=='e12c19954ca89e978f61c64e4ce74e58c4aa4b99b095a663613288fd1e99849d'
assert r['target_wall_seconds']==60 and r['actual_wall_seconds']>=57.5
assert r['target_exactly_met']==(r['actual_wall_seconds']<=60)
assert r['memory_16gb_gate']=='not_run'
print(json.dumps({'source_sha256':info['source_sha256'],'targeted_tests':16,'failures':0,'one_day_actual_session':r,'measurement_scope':'one-day regression among parallel integration tests, not isolated formal performance trial','mandatory_pe0_30days':'not_run','full_L2_four_rates':'not_run','eight_hour':'not_run','real_16GB':'not_run','full_M0':'not_accepted'},indent=2))
