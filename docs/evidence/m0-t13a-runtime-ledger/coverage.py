#!/usr/bin/env python3
"""Actual sampled frames, not process lifetime peak reconstruction."""
import collections,json,sys
from pathlib import Path
r=Path(sys.argv[1]);m=r/'model';heaps=[json.loads(s) for s in (m/'heaps.jsonl').read_text().splitlines()]
counts=collections.Counter(v['pid'] for v in heaps);assert all(n==1 for n in counts.values())
helper={mode:json.loads((m/mode/'result.json').read_text())['pid'] for mode in ['reference','before-reopen','compare']};frames=collections.Counter();last=None
for line in (r/'host-samples.jsonl').read_text().splitlines():
 last=json.loads(line)
 for pid in {v[0] for v in last['members']}:frames[pid]+=1
proof={'scope':'observed RSS frames by PID; all heap PIDs unique in this run, not an absolute peak proof','helper_coverage':{mode:{'pid':pid,'sampled_frames':frames[pid],'normal_heap_footer':any(v['pid']==pid and v['success'] for v in heaps)} for mode,pid in helper.items()},'short_helpers_without_RSS_frame':[pid for pid in helper.values() if frames[pid]==0],'last_frame_member_roles':[v[4] for v in last['members']],'whole_process_RSS_peaks_for_unsampled_helpers':'not_measured'}
assert set(proof['last_frame_member_roles'])=={'observer','sampler_ps'}
print(json.dumps(proof,indent=2))
