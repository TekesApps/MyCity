"""Address intersections and rounded named VM regions, never residual credits."""
import hashlib,json,re
from collections import Counter
from pathlib import Path
D=Path(__file__).resolve().parent;R=D.parents[2];terminal=json.loads((D/'terminal.json').read_text());reports=[]
for x in terminal['captures']:
 text=(R/x['stdout']).read_text();page=int(re.search(r'VM page size:\s+(\d+) bytes',text).group(1));regions=[]
 for line in text.splitlines():
  m=re.match(r'(.+?)\s+([0-9a-f]+)-([0-9a-f]+)\s+\[\s*([\d.]+)\s+([\d.]+)\s+([\d.]+)\s+([\d.]+)\s*\]',line)
  if m:regions.append(dict(type=m[1].strip(),start=int(m[2],16),end=int(m[3],16),reported_pages=dict(zip(['virtual','resident','dirty','swap'],m.group(4,5,6,7))),raw=line))
 ranges=x['topology']['ranges'];mapped=[];totals=Counter()
 for a in ranges:
  lo=a['start'];hi=lo+a['capacity'];parts=[]
  for region in regions:
   begin=max(lo,region['start']);end=min(hi,region['end'])
   if begin<end:parts.append(dict(start=begin,end=end,bytes=end-begin,type=region['type']))
  assert sum(v['bytes']for v in parts)==a['capacity'],(x['label'],x['role'],a,parts)
  totals[a['kind']]+=a['capacity'];mapped.append(dict(**a,intersections=parts))
 cb=x['context_before'];ca=x['context_after'];assert cb['admitted']==ca['admitted'] and cb['durable']==ca['durable'];assert x['identity_unchanged']and x['stable_phase_epoch_list_endpoints']
 reports.append(dict(label=x['label'],role=x['role'],pid=x['pid'],page_size=page,tool_begin=x['tool_begin_monotonic_ns'],tool_end=x['tool_end_monotonic_ns'],identity_unchanged=True,phase=x['label'],r07_epoch=cb['ack']['state']['epoch'],backing_list_epoch=x['topology']['header']['epoch'],phase_counters=[cb['admitted'],cb['durable']],range_totals=dict(totals),ranges=mapped,topology_sha256=x['topology']['sha256'],raw_vm_sha256=hashlib.sha256((R/x['stdout']).read_bytes()).hexdigest(),raw_footprint_before=x['identity_before']['last']['phys_footprint_bytes'],raw_footprint_after=x['identity_after']['last']['phys_footprint_bytes'],named_region_summary_lines=[l for l in text.splitlines()if l.startswith(('page table in kernel','MALLOC metadata','MALLOC_SMALL','MALLOC_LARGE','Stack ','Memory Tag 22','DefaultMallocZone'))],physical_attribution='exact byte address coverage only; mapping resident/dirty counts may span several owner ranges and are not additive physical credits'))
summary=[dict(label=x['label'],role=x['role'],ranges=len(x['ranges']),range_totals=x['range_totals'],types=dict(Counter(p['type']for r in x['ranges']for p in r['intersections'])),before=x['raw_footprint_before'],after=x['raw_footprint_after'],page_table_lines=[s for s in x['named_region_summary_lines']if s.startswith('page table in kernel')])for x in reports]
result=dict(source_sha256=terminal['source_sha256'],captures=reports,summary=summary,valid_idle_and_inflight=True,continuity='monotonic admitted/durable counters unchanged across each tool interval; external markers gate only admission, scans continue; no atomic group footprint claim',physical_credit_authorized=0,limits=['VM tools perturb targets and are outside native group; not formal performance evidence','Page table/summary page counts may be rounded; no exact byte credit from multiplication','Address ownership, resident/dirty pages, and whole footprint are distinct','Pool startup list is valid here because the actual L1 pool terminal report has zero grants; not an L2 dynamic list claim','Memory Tag22 and any remaining unowned regions remain unknown'])
(D/'address-audit.json').write_text(json.dumps(result,indent=2)+'\n');print(json.dumps(summary,indent=2))
