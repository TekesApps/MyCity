"""Independent original full closure/byte audit after L2 diagnostic terminal."""
import collections,hashlib,json,sys
from pathlib import Path
R=Path('/Users/frankqin/dev/MyCity');D=Path(__file__).resolve().parent;W=Path(sys.argv[1]);M=W/'model';B=M/'business/measurement';load=lambda p:json.loads(p.read_text());sha=lambda p:hashlib.sha256(p.read_bytes()).hexdigest()
source=load(D/'source.json')['source_sha256'];report=load(B/'report.json');assert report['source_sha256']==source
assert report['completed_hours']==720 and report['verified_targets']==723 and report['worker_windows']==724
assert report['entire_engine_including_metadata']=='equal_independent_normal_reference'
assert (B/'cpu-targets.jsonl').read_bytes()==(R/'work/t13a-role-arena-business-01/business/cpu-targets.jsonl').read_bytes()
world=B/'world';current=load(world/'CURRENT.json');rootp=world/f"root-{current['slot']}.json";assert sha(rootp)==current['root_sha256'];root=load(rootp)
p=root['checkpoint_v2'];idxp=world/f"world-index-{p['generation']}.json";assert sha(idxp)==p['sha256'];idx=load(idxp);p=idx['current'];image=world/f"world-{p['generation']}.json";assert image.stat().st_size==p['bytes'] and sha(image)==p['sha256'];assert image.read_bytes()==(B/'cpu-final-engine.json').read_bytes()
# Whole original target stream is unchanged; no metadata/capacity exclusions.
proof={'source_sha256':source,'original_hours':720,'original_targets':723,'worker_windows':724,'final_sequence':root['sequence'],'full_engine_bytes':image.stat().st_size,'full_engine_sha256':sha(image),'original_target_stream_equal_v85':True,'complete_engine_byte_equal_current_independent_cpu':True,'steady_window':report['steady_window'],'scope':'actual original full trajectory and latest published full Engine; no formal speed/steady acceptance'}
heaps=[json.loads(s) for s in (M/'heaps.jsonl').open()];assert all(h['success'] for h in heaps)
roles=collections.Counter(h['role'] for h in heaps);assert roles['steady-driver']==roles['steady-run']==roles['normal-pool-host']==1
proof['normal_heap_records']=len(heaps);proof['roles']=dict(roles);proof['all_recorded_normal_exits_successful']=True
poolrows=[json.loads(s) for s in (M/'normal-pool/events.jsonl').open()]
for r in poolrows:assert r['summary']['available_touched_bytes']+r['summary']['granted_capacity_bytes']==64*1024**2
last=poolrows[-1]['summary'];assert last['available_touched_bytes']==64*1024**2 and last['granted_capacity_bytes']==0 and last['workspace_grants']==last['workspace_returns'] and last['grants']==last['returns']
proof['pool_final']=last
print(json.dumps(proof,indent=2))
