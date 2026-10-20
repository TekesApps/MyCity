import collections,hashlib,json,sys
from pathlib import Path
root=Path(sys.argv[1]);observer=json.loads((root/'observer-report.json').read_text())
assert observer['status']=='failed' and observer['reason']=='system non-normal pressure exceeded 60 seconds' and observer['maximum_sampled_non_normal_pressure_seconds']>60
records=[json.loads(l) for l in (root/'model/heaps.jsonl').read_text().splitlines()]
assert records and all(v['success'] and v['role']=='compute-worker-bounded' for v in records)
assert len({(v['pid'],v['instance_unix_ns']) for v in records})==len(records)
complete=0;max_charged=0
for p in (root/'model/business/diagnostics').glob('worker-*/report.json'):
 d=json.loads(p.read_text());pid=int(p.parent.name.split('-')[1]);matching=[v for v in records if v['pid']==pid]
 # The last Complete may precede a protected forced stop; never invent its exit.
 if matching:assert len(matching)==1 and matching[0]['heap']['lifetime_peak_requested_bytes']>=d['whole_process_rust_heap']['lifetime_peak_requested_bytes'];complete+=1
 assert d['charged_diagnostics_bound_bytes']<=1024**2;max_charged=max(max_charged,d['charged_diagnostics_bound_bytes'])
result={'status':'protected_failed_attempt_retained_not_accepted','reason':observer['reason'],'wall_seconds':observer['actual_wall_seconds'],'complete_worker_exit_records':len(records),'paired_worker_reports':complete,'maximum_completed_worker_diagnostic_charge':max_charged,'missing_other_exit_records':'expected after SIGTERM; unmeasured not zero','formal_gate':'failed'}
print(json.dumps(result,indent=2))
