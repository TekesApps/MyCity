#!/usr/bin/env python3
"""Observed per-instance coverage. Reused PIDs are ambiguous, never silently merged."""
import collections,json,sys
from pathlib import Path
r=Path(sys.argv[1]);m=r/'model/business';heaps=[json.loads(s) for s in (r/'model/heaps.jsonl').read_text().splitlines()]
counts=collections.Counter(v['pid'] for v in heaps);frames=collections.Counter();rss=collections.defaultdict(int);missing=collections.Counter();last=None
for line in (r/'host-samples.jsonl').read_text().splitlines():
 last=json.loads(line)
 for pid,ppid,birth,value,role in last['members']:
  frames[pid]+=1;rss[pid]=max(rss[pid],value)
  if birth is None:missing[pid]+=1
instances=[];by_role=collections.defaultdict(collections.Counter)
for h in heaps:
 p=h['pid'];ambiguous=counts[p]>1;n=None if ambiguous else frames[p]
 row={'pid':p,'instance_unix_ns':h['instance_unix_ns'],'role':h['role'],'normal_heap_footer':h['success'],'sampled_frames':n,'sampled_rss_max_bytes':None if ambiguous or n==0 else rss[p],'sampled_footprint_missing_frames':None if ambiguous else missing[p],'PID_reused_not_attributable':ambiguous}
 instances.append(row);by_role[h['role']]['instances']+=1;by_role[h['role']]['ambiguous' if ambiguous else 'not_seen' if n==0 else 'seen']+=1
helper={}
for mode in ['reference','before-reopen','compare']:
 h=json.loads((m/mode/'result.json').read_text());key=(h['pid'],h['instance_unix_ns']);entry=[v for v in instances if (v['pid'],v['instance_unix_ns'])==key];assert len(entry)==1;helper[mode]=entry[0]
assert set(row[4] for row in last['members'])=={'observer','sampler_ps'}
proof={'scope':'actual observed frames; ambiguous reused PID not assigned to a process instance; sampled maxima are not absolute lifetime peaks','instances':instances,'role_coverage':{k:dict(v) for k,v in by_role.items()},'helper_coverage':helper,'short_helpers_without_RSS_frame':[v['pid'] for v in helper.values() if v['sampled_frames']==0],'ambiguous_helper_identities':[[v['pid'],v['instance_unix_ns']] for v in helper.values() if v['PID_reused_not_attributable']],'last_frame_member_roles':[v[4] for v in last['members']],'whole_process_absolute_RSS_peaks':'not_proven_by_periodic_sampling','unsampled_or_ambiguous_not_counted_as_zero_memory':True}
print(json.dumps(proof,indent=2))
