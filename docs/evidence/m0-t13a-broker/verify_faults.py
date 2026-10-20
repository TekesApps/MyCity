#!/usr/bin/env python3
"""Archive/ordering audit for real finite fault tests; no8h or physical-peak inference."""
import hashlib,json,zipfile
from pathlib import Path
D=Path(__file__).resolve().parent;info=json.loads((D/'source.json').read_text());manifest=json.loads((D/'fault-manifest.json').read_text());summaries=[]
with zipfile.ZipFile(D/'fault-artifacts.zip') as z:
 assert set(z.namelist())=={r['path'] for r in manifest}
 for r in manifest:
  b=z.read(r['path']);assert len(b)==r['bytes'] and hashlib.sha256(b).hexdigest()==r['sha256']
 for name in sorted(n for n in z.namelist() if n.endswith('/pool/ready.json')):
  root=name.removesuffix('ready.json');ready=json.loads(z.read(name));assert ready['source']==info['source_sha256']
  lines=z.read(root+'events.jsonl').splitlines();events=[json.loads(line) for line in lines];session=None;pending=None;active=None;last_gen=0;waits=0;returns=0
  assert all(len(line)+1<=1536 for line in lines)
  for e in events:
   k=e['kind'];s=e['summary'];assert s['available_touched_bytes']+s['granted_capacity_bytes']==67108864
   assert e['owner_actual_capacity_bytes']==s['available_touched_bytes']
   assert not {'key','worker_key','supervisor'}.intersection(e)
   if k=='broker_session_registered':
    assert session is None and pending is None and active is None and e['broker_generation']>last_gen
    last_gen=e['broker_generation'];session={'generation':last_gen,'pid':None}
   elif k=='broker_sim_bound':
    assert session and session['generation']==e['broker_generation'];assert session['pid'] in [None,e['broker_sim_pid']];session['pid']=e['broker_sim_pid']
   elif k=='grant_released_before_spawn':
    assert active is None and session and session['generation']==e['broker_generation'] and session['pid']==e['parent']==e['broker_sim_pid']
    active={'id':e['id'],'pid':None,'waited':False,'filled':False}
   elif k=='child_attached':
    assert active and active['id']==e['id'] and active['pid'] is None;active['pid']=e['pid']
   elif k=='worker_actual_slots_filled':
    assert active and active['pid']==e['pid']==e['worker_identity']['process_pid'] and not active['filled'];active['filled']=True
   elif k in ['owned_child_wait_success','owned_child_wait_failure']:
    assert active and active['pid']==e['pid'] and active['id']==e['id'] and not active['waited'];active['waited']=True;waits+=1
   elif k=='returned_after_wait_or_abort':
    assert active and active['waited'] and active['pid']==e['pid'] and active['id']==e['id'];active=None;returns+=1
   elif k=='broker_session_revoked_after_sim_wait':
    assert active is None and session and pending is None and e['broker_generation']==session['generation'];pending=session['generation'];session=None
   elif k=='broker_revoke_receipt_acknowledged':
    assert session is None and pending==e['broker_generation'];pending=None
   elif k=='broker_unspawned_cancelled':
    assert session and session['pid'] is None and active is None and session['generation']==e['broker_generation'];session=None
   else:assert k in ['created','parent_pipe_closed','periodic_balance_scan','rejected'],k
  report_name=root+'report.json';has_report=report_name in z.namelist()
  if has_report:
   report=json.loads(z.read(report_name));assert report['balanced_after_owned_wait'] and session is None and pending is None and active is None
   assert events[-1]['kind']=='parent_pipe_closed' and report['summary']['grants']==report['summary']['returns']==returns==waits
  else:
   assert ready['fault_injection']=='after-broker-spawn-pause' and active and active['filled'] and not active['waited'] and returns==waits==0
  summaries.append({'path':root,'fault':ready['fault_injection'],'normal_pool_report':has_report,'actual_wait_events':waits,'return_events':returns,'maximum_event_bytes':max(len(l)+1 for l in lines)})
assert len(summaries)==12 and sum(s['normal_pool_report'] for s in summaries)==11
print(json.dumps({'source_sha256':info['source_sha256'],'scope':'current-source real finite fault artifacts; actual wait path and cold results asserted by integration tests; no physical peaks inferred','archive_files_verified':len(manifest),'pools':summaries,'eight_hour':'not_run','full_business':'separate_run_required','real16GB':'not_run'},indent=2))
