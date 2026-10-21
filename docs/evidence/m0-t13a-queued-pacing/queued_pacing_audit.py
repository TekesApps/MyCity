#!/usr/bin/env python3
"""Offline v2 queued-deadline conservation; not a surrogate performance measurement."""
import json,sys
from pathlib import Path
M=Path(sys.argv[1])/'model';load=lambda p:json.loads(p.read_text());info=load(Path(__file__).resolve().parent/'source.json')
count=0;previous_due=None;first_scheduled=None;previous_interval=0;phase=0;rebase_shift=0;rebases=0;waits=0;late_total=0;max_late=0;last=None
with (M/'transcript.jsonl').open() as f:
 for line in f:
  try:x=json.loads(line)
  except json.JSONDecodeError:continue
  if x['request']['request']['command']!='run':continue
  r=x['response']['result'];s=r['status'];assert s['wall_pacing']=='volatile_ready_scheduled_release_v2' and s['provenance']['source_sha256']==info['source_sha256']
  assert r['outcome']['has_persistent_receipt'] and 'Ok' in r['outcome']['model'] and not r['already_recorded']
  interval=r['pacing_interval_ns'];seconds=x['request']['request']['envelope']['actions'][0]['command']['seconds']
  assert interval==seconds*60000*1000000//86400
  due=s['next_release_in_session_ns'];scheduled=due-interval
  actual_elapsed=r['pacing_release_elapsed_ns'];scheduled_elapsed=r['pacing_scheduled_elapsed_ns'];lateness=r['pacing_release_lateness_ns']
  assert actual_elapsed>=scheduled_elapsed and actual_elapsed-scheduled_elapsed==lateness
  if first_scheduled is None:first_scheduled=scheduled
  else:
   phase+=previous_interval;gap=scheduled-previous_due;assert gap>=0
   if r['pacing_wait_ns']>0:
    assert gap==0,'early-ready request must preserve prior scheduled deadline'
    waits+=1
   if gap>0:assert r['pacing_wait_ns']==0;rebases+=1;rebase_shift+=gap
  previous_due=due;previous_interval=interval;late_total+=lateness;max_late=max(max_late,lateness);count+=1;last=x
assert count and last is not None
assert scheduled-first_scheduled-phase==rebase_shift
report=load(M/'report.json') if (M/'report.json').exists() else None
print(json.dumps({'source_sha256':info['source_sha256'],'schema':'queued-pacing-audit-v2','observed_run_responses':count,'complete_30day_business_responses':count==722 and last['response']['result']['status']['committed_game_seconds']==2592000,'final_report_present':report is not None,'positive_wait_requests_after_first':waits,'late_ready_rebases':rebases,'scheduled_phase_shift_ns':rebase_shift,'sum_actual_release_lateness_ns':late_total,'maximum_actual_release_lateness_ns':max_late,'last_actual_release_lateness_ns':lateness,'first_scheduled_in_session_ns':first_scheduled,'last_scheduled_in_session_ns':scheduled,'nominal_phase_ns':phase,'exact_integer_phase_conservation':True,'all_early_ready_releases_preserve_phase':True,'speed_target_met':None if report is None else report['speed_target_met'],'interpretation':'actual lateness retained; no subtraction from operating wall time; full performance verdict comes from real report/verify','eight_hour':'not_run','real_16GB':'not_run','full_M0':'not_accepted'},indent=2))
