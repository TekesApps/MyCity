from pathlib import Path
import json,hashlib,subprocess,re,datetime
r=Path('/Users/frankqin/dev/MyCity');w=r/'work/water-treatment-runtime-20261025';d=r/'docs/evidence/water-treatment-runtime-20261025'
reg=json.loads((r/'docs/inputs/issue-ledger/registry.json').read_text());flow=json.loads((r/'docs/inputs/issue-ledger/execution-flow.json').read_text());rows={x['issue']:x for x in reg['issues']};actual=json.loads((w/'near-issues-connector-readback193.json').read_text())+json.loads((w/'parents-connector-readback193.json').read_text());by={x['issue_number']:x for x in actual}
assert not flow['active_shared_work'] and not flow['active_independent_preparations'];assert flow['limits']['shared_core_lanes']==1 and flow['limits']['independent_preparations']==2
assert flow['window_completed']=={'new_accepted_business':[132,133],'historical_backfill_business':[],'defects_closed':[148]}
assert len(flow['current_backlog']['source_delivered_pending_integration'])==21 and flow['current_backlog']['source_delivered_pending_validation']==[]
for n,x in by.items():assert rows[n]['issue_body_sha256']==hashlib.sha256(x['body'].encode()).hexdigest(),n
assert by[133]['state']=='closed' and any(x['name']=='stage:done' for x in by[133]['labels']);assert by[92]['state']=='open' and '- [x] #133' in by[92]['body'];assert rows[133]['stage']=='完成' and rows[133]['execution_state']=='completed:closed'
for n in [132,148]:assert by[n]['state']=='closed'
for n in [134,135,136,137]:assert by[n]['state']=='open'
parents={x['issue']:([x['parent']]if x['parent']else[])for x in reg['issues']};edges={x['consumer']:x['requires']for x in reg['capability_edges']}
for graph in [parents,edges]:
 seen=set();stack=set()
 def visit(n):
  assert n in rows and n not in stack,n
  if n in seen:return
  stack.add(n)
  for nxt in graph.get(n,[]):visit(nxt)
  stack.remove(n);seen.add(n)
 for n in rows:visit(n)
for n,x in json.loads((w/'preserved-wip.json').read_text()).items():assert hashlib.sha256((r/n).read_bytes()).hexdigest()==x['sha256'],n
closure=json.loads((d/'closure.json').read_text());pub=json.loads((d/'publication192-verified.json').read_text());assert closure['implementation_commit']==pub['commit']==subprocess.check_output(['git','rev-parse','HEAD'],cwd=r,text=True).strip();assert pub['tag']=='v0.5.192' and pub['tag_annotated_object_verified']
for x in flow['near_term_contracts']:
 if x['issue']==133:assert x['timing']['accepted_at']==closure['accepted_at'] and x['timing']['issue_closed_at']==closure['closed_at'] and not x['new_write_authority']
for f in ['README.md','docs/execution-current.md','docs/issue-execution-ledger.md','docs/water-quality-and-treatment-runtime.md','docs/evidence/water-treatment-runtime-20261025/STATUS.md']:
 for link in re.findall(r'\]\(([^)]+)\)',(r/f).read_text()):
  if '://'in link or link.startswith('#'):continue
  assert ((r/f).parent/link.split('#')[0]).is_file(),(f,link)
assert '仓库发布版本：`v0.5.193`'in(r/'README.md').read_text()
for f in d.glob('*.json'):json.loads(f.read_text())
size=sum(f.stat().st_size for root in [w,r/'work/w33']for f in root.rglob('*')if f.is_file());assert size<2147483648
changed=subprocess.check_output(['git','diff','--name-only'],cwd=r,text=True).splitlines();protected=set(json.loads((w/'preserved-wip.json').read_text()));assert all(n.startswith('docs/')or n=='README.md' for n in changed if n not in protected)
subprocess.run(['git','diff','--check','--','README.md','docs/execution-current.md','docs/core-task-dispatch-current.md','docs/issue-execution-ledger.md','docs/inputs/issue-ledger','docs/evidence/water-treatment-runtime-20261025'],cwd=r,check=True)
result={'at':datetime.datetime.now(datetime.timezone.utc).isoformat(),'actual_closed_business':[132,133],'historical_backfill':[],'closed_defects':[148],'133_closed_92_open_and_done_label_verified':True,'registry10_fresh_body_hashes_and_parent_list_match':True,'graphs_acyclic':True,'timings_not_invented':True,'actual_WIP':[0,0],'unrelated_WIP24_preserved':True,'current_game_code_unchanged_since192':True,'source_target_work':'original HP P900 only','combined_retained_logical_bytes':size,'retention_cap_bytes':2147483648,'publication':'game192 REST verified; ledger193 only','whole_goal':'active; full nonUI not complete'}
(w/'closure193-verified.json').write_text(json.dumps(result,indent=2)+'\n');print(json.dumps(result))
