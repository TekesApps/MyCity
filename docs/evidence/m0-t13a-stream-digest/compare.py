#!/usr/bin/env python3
"""Before/after full frozen workload diagnostics, not a pooled formal gate."""
import json
from pathlib import Path
D=Path(__file__).resolve().parent;old=D.parent/'m0-t13a-joint-recheck';load=lambda p:json.loads(p.read_text())
a=load(old/'latency.json');b=load(D/'latency.json')
select=lambda x,role:next(s for s in x['strata'] if s['record_state']=='sealed' and s['producer']=='sim' and s['role']==role and s['class']=='L2_background_capture_admit_cpu')
rows={r:{'before':select(a,r),'after':select(b,r)} for r in ['execution','quote']}
assert rows['execution']['after']['observed_p95_p99_thresholds_met'] and not rows['quote']['after']['observed_p95_p99_thresholds_met']
va=load(old/'verification.json');vb=load(D/'verification.json');assert va['receipts']==vb['receipts']==3013 and va['all_724_logical_input_candidate_hashes_equal_v48'] and vb['all_724_logical_input_candidate_hashes_equal_v48']
assert va['L1_scans']==vb['L1_scans']==600 and va['L1_snapshots']==vb['L1_snapshots']==2
print(json.dumps({'scope':'independent original full-joint smoke runs; not source pooling or formal gate','before_source':a['source_sha256'],'after_source':b['source_sha256'],'capture_admit_by_role':rows,'business_wall_seconds':{'before':va['clock']['actual_wall_seconds'],'after':vb['clock']['actual_wall_seconds']},'dynamic_fault_jobs':{'before':[va['clock']['retry_job'],va['clock']['committed_job']],'after':[vb['clock']['retry_job'],vb['clock']['committed_job']]},'sample_and_background_limitation':'same frozen load and observer, distinct source/run; fault-selected sealed populations and external background differ, so no strict isolated causal or formal statistical conclusion','formal_10class':'not_accepted; quote p95 still exceeds20ms, missing L0/rare and steady protocol','normal64_common8':'not_accepted','eight_hour_real16GB':'not_run'},indent=2))
