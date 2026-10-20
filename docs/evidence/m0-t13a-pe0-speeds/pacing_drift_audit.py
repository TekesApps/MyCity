#!/usr/bin/env python3
"""Offline integer timing audit; partial trajectories never imply acceptance."""
import json,sys
from pathlib import Path
M=Path(sys.argv[1])/'model';count=0;last_row=None;previous_deadline=None;first_release=None;phase_ns=0;previous_interval=0;late_ns=0;wake_ns=0;waited=0;late_ready=0;target=0;last_release=0
with (M/'transcript.jsonl').open() as f:
 for line in f:
  try:x=json.loads(line)
  except json.JSONDecodeError:continue
  if x['request']['request']['command']!='run':continue
  r=x['response']['result'];assert r['outcome']['has_persistent_receipt'] and 'Ok' in r['outcome']['model'] and not r['already_recorded']
  interval=r['pacing_interval_ns'];deadline=r['status']['next_release_in_session_ns'];released=deadline-interval
  if first_release is None:first_release=released
  else:
   phase_ns+=previous_interval;gap=released-previous_deadline;assert gap>=0
   if r['pacing_wait_ns']>0:wake_ns+=gap;waited+=1
   else:late_ns+=gap;late_ready+=1
  previous_deadline=deadline;previous_interval=interval;last_release=released;target=r['status']['committed_game_seconds'];count+=1;last_row=x
assert count and last_row is not None
assert last_release-first_release-phase_ns==wake_ns+late_ns
report=json.loads((M/'report.json').read_text()) if (M/'report.json').exists() else None
result={'source_sha256':last_row['response']['result']['status']['provenance']['source_sha256'],'rate':'60','observed_run_responses':count,'last_game_seconds':target,'complete_30day_business_responses':count==722 and target==2592000,'final_report_present':report is not None,'proof_scope':'integer conservation of original actual-release-anchored pacing; no model execution, no new tolerance','attribution':'post-wait release delay includes wake scheduling and adjacent code overhead; not isolated OS sleep cost','positive_sleep_requests_after_first':waited,'no_sleep_late_ready_requests_after_first':late_ready,'accumulated_wake_phase_shift_ns':wake_ns,'accumulated_late_ready_phase_shift_ns':late_ns,'first_release_in_session_ns':first_release,'last_release_in_session_ns':last_release,'ideal_phase_from_previous_intervals_ns':phase_ns,'exact_integer_phase_conservation':True,'last_response_operating_seconds':last_row['completed_seconds'],'speed_target_met':None if report is None else report['speed_target_met'],'eight_hour':'not_run','real_16GB':'not_run','full_M0':'not_accepted'}
print(json.dumps(result,indent=2))
