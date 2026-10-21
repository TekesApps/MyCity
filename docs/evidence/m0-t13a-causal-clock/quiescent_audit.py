"""Offline resource matching. Full uncertainty and actual idle gaps only.
No inferred frames, no zero substitution, no automatic leakage acceptance.
"""
import json,sys
from decimal import Decimal,ROUND_FLOOR,ROUND_CEILING
from pathlib import Path
NS=10**9
def sec_bounds(value):
 x=Decimal(str(value))*NS
 return int(x.to_integral_value(rounding=ROUND_FLOOR))-1,int(x.to_integral_value(rounding=ROUND_CEILING))+1

def inside(frame,origin,gap):
 lo,hi=origin;a,b=frame['clock_begin_ns'],frame['clock_end_ns']
 assert a<=b and lo<=hi
 return a-hi>=gap[0] and b-lo<=gap[1]

def overlaps(frame,span):
 return frame['clock_end_ns']>=span[0] and frame['clock_begin_ns']<=span[1]

def layout_exclusions(events,origin):
 # layout started Instant precedes every scan begin; event elapsed is read
 # after scan-end (and after allocator clock, where present). These two
 # inequalities bound its offset from the independently mapped heap clock.
 lowers=[];uppers=[]
 for row in events:
  if 'scan' not in row:continue
  c=row.get('allocator_after_scan_clock',row['clock_end'])
  lowers.append(c['elapsed_ns']-sec_bounds(row['elapsed_seconds'])[1])
  uppers.append(row['clock_begin']['elapsed_ns'])
 if not lowers:return [],None
 delta=(max(lowers),min(uppers));assert delta[0]<=delta[1],delta
 active={};spans=[]
 for row in events:
  if 'snapshot_admitted' in row:
   g=row['snapshot_admitted']['generation'];assert g not in active
   active[g]=origin[0]+delta[0]+sec_bounds(row['elapsed_seconds'])[0]
  if 'snapshot_durable' in row:
   g=row['snapshot_durable']['generation'];assert g in active
   spans.append((active.pop(g),origin[1]+delta[1]+sec_bounds(row['elapsed_seconds'])[1]))
 for start in active.values():spans.append((start,10**40))
 return spans,delta

def idle_boundaries(transcript,warmup_ns):
 offset=None;days={};gaps=[];pending=None
 for row in transcript:
  data=row['data']
  if 'experiment_origin_measurement_ns' in data:
   assert offset is None;offset=warmup_ns+data['experiment_origin_measurement_ns']
  if 'steady_quiescent_day_boundary' in data:
   days[data['steady_quiescent_day_boundary']]=data
  if offset is None:continue
  lo,hi=sec_bounds(row['elapsed_seconds']);lo+=offset;hi+=offset
  if 'request' in data and 'command' in data and pending:
   pending['gap_driver_ns'][1]=lo
   assert pending['gap_driver_ns'][0]<=lo
   gaps.append(pending);pending=None
  if 'steady_quiescent_day_released' in data:
   assert pending is None
   day=data['steady_quiescent_day_released'];assert day in days
   pending={'day':day,'phase':data['phase'],'game_target':data['game_target'],
     'status':days[day]['status'],'gap_driver_ns':[hi,None]}
 if pending:pending['gap_driver_ns'][1]=None;gaps.append(pending)
 return days,gaps

def main(root):
 load=lambda p:json.loads(p.read_text());M=root/'model';s=load(root/'observer-clock-map.json');d=load(M/'clock-map.json');l=load(M/'layout-clock-map.json');report=load(M/'business/report.json')
 origin=(s['supervisor_origin_observer_ns_min']+d['driver_origin_parent_ns_min'],s['supervisor_origin_observer_ns_max']+d['driver_origin_parent_ns_max'])
 layout_origin=(s['supervisor_origin_observer_ns_min']+l['layout_origin_parent_ns_min'],s['supervisor_origin_observer_ns_max']+l['layout_origin_parent_ns_max'])
 events=[json.loads(x) for x in (M/'layout/events.jsonl').open()]
 exclusions,delta=layout_exclusions(events,layout_origin)
 transcript=(json.loads(x) for x in (M/'business/measurement/transcript.jsonl').open())
 days,gaps=idle_boundaries(transcript,round(report['warmup_seconds']*NS))
 summaries=[]
 for gap in gaps:
  status=gap.pop('status');runtime=status['runtime'];allocator=runtime.get('allocator_boundary')
  codec=runtime['sim_codec'];gap.update(sequence=status['sequence'],timeline=status['timeline'],allocator_boundary=allocator,
    store_owned_bytes=runtime['store_owned_bytes'],diagnostics_owned_bytes=runtime['diagnostics_owned_bytes'],sim_air_input_owned_bytes=runtime['sim_air_input_owned_bytes'],
    codec_balanced=codec['full_balance_proven'] and codec['lent_capacity_bytes']==0,
    eligible_complete_frames=0,excluded_snapshot_frames=0,incomplete_frames=0,pid_instance_issues=0,
    group_rss_min=None,group_rss_max=None,group_footprint_min=None,group_footprint_max=None,
    sim_rss_min=None,sim_rss_max=None,sim_footprint_min=None,sim_footprint_max=None,observed_sim_mach_births=[])
  summaries.append(gap)
 for line in (root/'host-samples.jsonl').open():
  frame=json.loads(line)
  for gap in summaries:
   if gap['gap_driver_ns'][1] is None or not inside(frame,origin,gap['gap_driver_ns']):continue
   if any(overlaps(frame,x) for x in exclusions):gap['excluded_snapshot_frames']+=1;continue
   if not frame.get('group_capture_complete',False) or frame.get('missing_footprint',1) or frame.get('missing_rss',1):gap['incomplete_frames']+=1;continue
   a=gap['allocator_boundary']
   if a is None:gap['pid_instance_issues']+=1;continue
   begin,end=a['clock_begin'],a['clock_end'];pid=begin['pid']
   if (pid,begin['instance_unix_ns'])!=(end['pid'],end['instance_unix_ns']):gap['pid_instance_issues']+=1;continue
   members=[m for m in frame['members'] if m[0]==pid]
   feet=[m for m in frame.get('member_footprints_pid_birth_bytes',[]) if m[0]==pid]
   if len(members)!=1 or len(feet)!=1 or members[0][2] is None or feet[0][1]!=members[0][2] or feet[0][2] is None:
    gap['pid_instance_issues']+=1;continue
   if members[0][2] not in gap['observed_sim_mach_births']:gap['observed_sim_mach_births'].append(members[0][2])
   values={'group_rss':frame['rss_bytes'],'group_footprint':frame['phys_footprint_bytes'],'sim_rss':members[0][3],'sim_footprint':feet[0][2]}
   gap['eligible_complete_frames']+=1
   for key,value in values.items():
    for suffix,operation in [('min',min),('max',max)]:
     field=key+'_'+suffix;gap[field]=value if gap[field] is None else operation(gap[field],value)
 result={'schema':'mycity-quiescent-boundaries-v1','source_sha256':report['source_sha256'],'driver_origin_observer_ns':origin,'layout_started_offset_from_heap_clock_ns':delta,'snapshot_exclusion_intervals_observer_ns':exclusions,'day_status_markers':len(days),'release_markers':len(gaps),'boundaries':summaries,'current_internal_drift':None,'leakage_gate':'not_accepted; actual current counters and complete matching frames are inputs, history/role identity and same-phase growth attribution still require audit; no peak subtraction','original_tolerances_unchanged':True,'eight_hour':'not_run','real16GB':'not_run'}
 counters=[x for x in summaries if x['allocator_boundary']]
 if counters:
  first,last=counters[0],counters[-1];a,b=first['allocator_boundary']['snapshot'],last['allocator_boundary']['snapshot']
  result['current_internal_drift']={'first_day':first['day'],'last_day':last['day'],'current_requested_delta_bytes':b['current_requested_bytes']-a['current_requested_bytes'],'arena_used_delta_bytes':b['arena_used_bytes']-a['arena_used_bytes'],'arena_free_delta_bytes':b['arena_free_bytes']-a['arena_free_bytes'],'bootstrap_quarantined_delta_bytes':b['bootstrap_quarantined_bytes']-a['bootstrap_quarantined_bytes'],'store_owned_delta_bytes':last['store_owned_bytes']-first['store_owned_bytes'],'diagnostics_owned_delta_bytes':last['diagnostics_owned_bytes']-first['diagnostics_owned_bytes'],'air_input_owned_delta_bytes':last['sim_air_input_owned_bytes']-first['sim_air_input_owned_bytes'],'scope':'observed current values only; Store/codec/diag are inside fixed role, not additional grants; positive residual never automatically excused'}
 print(json.dumps(result,indent=2))
if __name__=='__main__':main(Path(sys.argv[1]))
