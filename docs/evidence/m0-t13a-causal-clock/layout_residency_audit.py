"""Match actual L1 allocator samples to complete idle physical captures.
Offline attribution evidence only; no allocator/RSS subtraction passes a gate.
"""
import json,sys
from pathlib import Path
from quiescent_audit import inside,overlaps,layout_exclusions

def main(root):
 load=lambda p:json.loads(p.read_text());M=root/'model';s=load(root/'observer-clock-map.json');l=load(M/'layout-clock-map.json')
 origin=(s['supervisor_origin_observer_ns_min']+l['layout_origin_parent_ns_min'],s['supervisor_origin_observer_ns_max']+l['layout_origin_parent_ns_max'])
 events=[json.loads(x) for x in (M/'layout/events.jsonl').open()];exclusions,delta=layout_exclusions(events,origin)
 scans=[x for x in events if 'scan' in x];rows=[]
 for a,b in zip(scans,scans[1:]):
  clock=a.get('allocator_after_scan_clock')
  if clock is None:continue
  assert (clock['pid'],clock['instance_unix_ns'])==(l['pid'],l['instance_unix_ns'])
  if a['snapshot_inflight']:continue
  row={'iteration':a['scan']['iteration'],'snapshot_inflight':False,'allocator':a['allocator_after_scan'],
    'gap_layout_heap_ns':[clock['elapsed_ns'],b['clock_begin']['elapsed_ns']],
    'eligible_complete_frames':0,'incomplete_frames':0,'snapshot_excluded_frames':0,
    'layout_rss_min':None,'layout_rss_max':None,'layout_footprint_min':None,'layout_footprint_max':None,
    'group_rss_max':None,'group_footprint_max':None,'member_ranges':{}}
  if row['gap_layout_heap_ns'][0]<=row['gap_layout_heap_ns'][1]:rows.append(row)
 for line in (root/'host-samples.jsonl').open():
  f=json.loads(line)
  for row in rows:
   if not inside(f,origin,row['gap_layout_heap_ns']):continue
   if any(overlaps(f,span) for span in exclusions):row['snapshot_excluded_frames']+=1;continue
   if not f.get('group_capture_complete',False):row['incomplete_frames']+=1;continue
   members=[m for m in f['members'] if m[0]==l['pid']]
   feet=[m for m in f.get('member_footprints_pid_birth_bytes',[]) if m[0]==l['pid']]
   if len(members)!=1 or len(feet)!=1 or members[0][2] is None or feet[0][1]!=members[0][2] or feet[0][2] is None:
    row['incomplete_frames']+=1;continue
   row['eligible_complete_frames']+=1
   values={'layout_rss':members[0][3],'layout_footprint':feet[0][2]}
   for name,value in values.items():
    for suffix,fn in [('min',min),('max',max)]:
     key=name+'_'+suffix;row[key]=value if row[key] is None else fn(row[key],value)
   for name in ('rss','footprint'):
    key='group_'+name+'_max';value=f['rss_bytes' if name=='rss' else 'phys_footprint_bytes'];row[key]=value if row[key] is None else max(row[key],value)
   foot_by_pid={m[0]:m for m in f['member_footprints_pid_birth_bytes']}
   for m in f['members']:
    key=str(m[0])+':'+str(m[2]);foot=foot_by_pid[m[0]][2]
    r=row['member_ranges'].setdefault(key,{'role':m[-1],'rss_min':m[3],'rss_max':m[3],'footprint_min':foot,'footprint_max':foot})
    r['rss_min']=min(r['rss_min'],m[3]);r['rss_max']=max(r['rss_max'],m[3]);r['footprint_min']=min(r['footprint_min'],foot);r['footprint_max']=max(r['footprint_max'],foot)
 print(json.dumps({'schema':'mycity-layout-residency-v1','mapped_pid':l['pid'],'mapped_instance_unix_ns':l['instance_unix_ns'],'origin_observer_ns':origin,'layout_started_offset_from_heap_clock_ns':delta,'all_original_scans':len(scans),'snapshot_exclusion_intervals':exclusions,'same_phase_samples':rows,'gate':'not_accepted; actual current allocation and physical capture reported separately; no claim System cache is the cause without ownership evidence','scope':'No dropped records/pressure/frequency; no interpolated resources or zero imputation'},indent=2))
if __name__=='__main__':main(Path(sys.argv[1]))
