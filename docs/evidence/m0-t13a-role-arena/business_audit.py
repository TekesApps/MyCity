"""Independent frozen business/reference/full Engine and real pool audit.
No L1/physical/steady acceptance inferred from this functional run.
"""
import collections,hashlib,json,zipfile
from pathlib import Path
D=Path(__file__).resolve().parent;R=D.parents[2];W=R/'work/t13a-role-arena-business-01';B=W/'business';MiB=1024**2
load=lambda p:json.loads(p.read_text());source=load(D/'arena-source.json')['source_sha256']
assert load(D/'arena-terminal.json')['functional_original_full_30day_verified']
b=load(B/'report.json');s=load(W/'report.json')
assert b['completed_hours']==720 and b['verified_targets']==723 and b['worker_windows']==724
assert b['entire_engine_including_metadata']=='equal_independent_normal_reference'
assert b['source_sha256']==s['source_sha256']==source
with zipfile.ZipFile(D.parent/'m0-t13a-broker/t13a3m-functional-01/artifacts.zip') as z:
 for name in ['cpu-final-engine.json','cpu-targets.jsonl']:
  assert (B/name).read_bytes()==z.read('model/business/'+name)
c=load(B/'world/CURRENT.json');raw=(B/'world'/f"root-{c['slot']}.json").read_bytes();assert hashlib.sha256(raw).hexdigest()==c['root_sha256'];root=json.loads(raw)
p=root['checkpoint_v2'];idxraw=(B/'world'/f"world-index-{p['generation']}.json").read_bytes();assert hashlib.sha256(idxraw).hexdigest()==p['sha256'];idx=json.loads(idxraw)
f=B/'world'/f"world-{idx['current']['generation']}.json";assert f.read_bytes()==(B/'cpu-final-engine.json').read_bytes()
assert root['sequence']==3011
rows=[json.loads(l) for l in (W/'normal-pool/events.jsonl').open()];by=collections.defaultdict(list)
for x in rows:
 assert x['summary']['available_touched_bytes']+x['summary']['granted_capacity_bytes']==64*MiB
 if x.get('schema')=='mycity-normal-role-arena-v2':by[x['generation']].append(x)
assert len(by)==5
assert collections.Counter(es[0]['owner_kind'] for es in by.values())=={'model':1,'sim_codec':2,'helper_compare':2}
heaps=[json.loads(l) for l in (W/'heaps.jsonl').open()];assert all(x['success'] and x['source_sha256']==source for x in heaps)
arenas=[]
for g,es in by.items():
 assert [x['phase'] for x in es]==['prepared','filled','retiring','returned']
 filled=es[1];a=filled['arena']['activation'];i=filled['identity'];cap={'model':20,'sim_codec':59,'helper_compare':6}[filled['owner_kind']]*MiB
 assert a['pid']==i['process_pid'] and a['grant_bytes']==cap
 assert a['arena_bytes']==a['initialized_bytes'] and a['arena_bytes']+a['bootstrap_charged_bytes']+a['adapter_header_bytes']==cap
 h=next(x for x in heaps if (x['pid'],x['instance_unix_ns'])==(i['process_pid'],i['process_instance_unix_ns']))
 assert h['role_arena']['activation']==a and h['role_arena']['allocation_failures']==0
 assert h['role_arena']['lifetime_peak_requested_bytes']<=cap
 assert es[3]['pool_touched_bytes']==64*MiB
 arenas.append(dict(kind=filled['owner_kind'],grant_bytes=cap,initialized_arena_bytes=a['arena_bytes'],peak_requested_bytes=h['role_arena']['lifetime_peak_requested_bytes']))
pool=load(W/'normal-pool/report.json');summary=pool['summary']
assert pool['balanced_after_owned_wait'] and summary['workspace_grants']==summary['workspace_fills']==summary['workspace_returns']==5
assert summary['grants']==summary['returns']==summary['fill_acknowledgements']==724 and summary['available_touched_bytes']==64*MiB
out=dict(source_sha256=source,whole_business_seconds=s['whole_wall_seconds'],operating_seconds=b['operating_wall_seconds'],cold_ready_seconds=b['cold_ready_seconds'],sequence=root['sequence'],hours=720,targets=723,workers=724,full_canonical_engine_equal_original_v48=True,all_original_target_bytes_equal=True,role_arenas=arenas,normal_heap_records=len(heaps),normal_pool_returned_bytes=summary['available_touched_bytes'],physical_joint_memory='not_tested',formal_performance='not_accepted',eight_hour='not_run',real_16GB='not_run')
(D/'business-audit.json').write_text(json.dumps(out,indent=2)+'\n');print(json.dumps(out,indent=2))
