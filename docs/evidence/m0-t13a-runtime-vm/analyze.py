"""Conservative address attribution. Rounded vmmap pages are never exact credits."""
import hashlib,json,re
from decimal import Decimal
from pathlib import Path
D=Path(__file__).resolve().parent;R=D.parents[2];W=R/'work/runtime-vm-l1-01/observed'
def load(p):return json.loads(p.read_text())
reports=[]
for path in sorted(D.glob('*-*-*.json')):
 if path.name.endswith('trigger.json'):continue
 x=load(path)
 if 'tool_begin_monotonic_ns' not in x:continue
 text=(R/x['stdout']).read_text();match=re.search(r'VM page size:\s+(\d+) bytes',text);assert match
 page=int(match[1]);regions=[]
 for line in text.splitlines():
  m=re.match(r'(.+?)\s+([0-9a-f]+)-([0-9a-f]+)\s+\[\s*([\d.]+)\s+([\d.]+)\s+([\d.]+)\s+([\d.]+)\s*\]',line)
  if m:regions.append(dict(type=m[1].strip(),start=int(m[2],16),end=int(m[3],16),reported_pages=dict(zip(['virtual','resident','dirty','swap'],m.group(4,5,6,7))),raw=line))
 a=x['event_before'];b=x['event_after'];before_phase='inflight' if a['snapshot_inflight'] else 'idle';after_phase='inflight' if b['snapshot_inflight'] else 'idle';phase=before_phase if before_phase==after_phase else 'mixed'
 record=dict(role=x['role'],pid=x['pid'],label=x['label'],phase_from_bracketing_events=phase,phase_continuity='endpoints only; independent event interval must exclude transition before treating continuous',identity_unchanged=x['identity_unchanged'],vmmap_exit=x['exit_code'],page_size=page,begin=x['tool_begin_monotonic_ns'],end=x['tool_end_monotonic_ns'],usage_before=x['before']['last'],usage_after=x['after']['last'],regions=regions,raw_sha256=hashlib.sha256((R/x['stdout']).read_bytes()).hexdigest())
 if x['role']=='layout':
  activation=a['allocator_after_scan']['activation'];lo=activation['arena_start'];hi=lo+activation['arena_bytes'];intersections=[]
  for region in regions:
   start=max(lo,region['start']);end=min(hi,region['end'])
   if start<end:intersections.append(dict(start=start,end=end,bytes=end-start,region_type=region['type'],mapping_start=region['start'],mapping_end=region['end']))
  assert sum(v['bytes'] for v in intersections)==activation['arena_bytes']
  assert all(v['region_type']=='MALLOC_LARGE' for v in intersections)
  record['arena']=dict(activation=activation,intersections=intersections,all_arena_bytes_mapped=True,physical_attribution='mapping intersection proves address ownership only; residency counts are mapping-level and cannot split head/tail exact physical charges',bootstrap='activation baseline and live/quarantined counters retained in events; no native ownership inferred')
 record['runtime_candidates']=[r for r in regions if r['type'] in ['MALLOC metadata','Stack','Memory Tag 22','Kernel Alloc Once','shared memory']]
 record['page_table_raw_lines']=[line for line in text.splitlines() if line.startswith('page table in kernel')]
 reports.append(record)
intervals={}
for label in ['inflight','idle']:
 xs=[x for x in reports if x['label']==label];lo=max(x['begin'] for x in xs);hi=min(x['end'] for x in xs)
 intervals[label]=dict(begin=lo,end=hi,all_tool_intervals_overlap=lo<=hi,statement='no summing per-process maxima; observations are perturbed, asynchronous and not atomic')
report=dict(schema='mycity-runtime-vm-address-audit-v1',captures=reports,intervals=intervals,source_sha256=load(D/'intent.json')['source_sha256'],snapshot_report=load(W/'model/layout/report.json'),formal_acceptance='not_accepted',replacement_bytes_authorized=0,limits=['VM tools are outside observer tree, separately instrumented by coordinator; their disturbance invalidates formal timing claims.','Readonly/shared mappings and compressed pages not counted repeatedly or converted to padding credits.','VM page table numbers may be rounded; actual rusage footprint is distinct.','Memory Tag 22 purpose is unknown, no credit.','Only first in-flight and idle captured; final snapshot group not acquired.','No evidence from 32GB host establishes real16GB acceptance.'])
(D/'address-audit.json').write_text(json.dumps(report,indent=2)+'\n')
print(json.dumps(dict(captures=len(reports),phase=[(x['label'],x['role'],x['phase_from_bracketing_events']) for x in reports],intervals=intervals,snapshots=len(report['snapshot_report']['snapshots']),replacement_bytes_authorized=0),indent=2))
