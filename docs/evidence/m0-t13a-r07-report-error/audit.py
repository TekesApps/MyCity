"""Offline raw identity/epoch/reading correlation, not an atomic-snapshot claim."""
import json
from pathlib import Path
D=Path(__file__).resolve().parent;R=D.parents[2];W=R/'work/r07-report-error-l1-01';root=W/'observed'
frames=[json.loads(l) for l in (root/'host-samples.jsonl').read_text().splitlines()];ev=[json.loads(l) for l in (root/'model/layout/events.jsonl').read_text().splitlines()]
readings={f['r07_reading']['sequence']:(i,f['r07_reading']) for i,f in enumerate(frames) if f.get('r07_reading')}
transitions=[];unknown=[];last=None;actual=0;complete=0;paused=0
for i,f in enumerate(frames):
 if f['group_capture_complete']:complete+=1
 if f['r07_provider_paused']:paused+=1
 a=f.get('r07_applied')
 if not a:continue
 s=a['state'];assert s['mapped_bytes']+s['replaced_bytes']+s['arena_tail_bytes']==s['logical_bytes'];assert s['mapped_bytes']%65536==0
 assert s['replaced_bytes']<=a['permitted_bytes']//65536*65536
 owner=a['owner'];assert any(m[0]==owner['pid'] and m[2]==owner['start_abstime'] for m in f['members'])
 key=(owner['pid'],owner['start_abstime'],s['epoch'],s['replaced_bytes'])
 if key!=last:transitions.append(dict(frame=i,relative_seconds=f['t'],owner=owner,state=s,reason=a['reason'],coverage=f['r07_coverage_complete']));last=key
 if s['replaced_bytes']:
  actual+=1;seq=s['reading_sequence'];prior=readings.get(seq)
  if prior is None:unknown.append(dict(frame=i,sequence=seq,reason='publication not present in raw before ack observation'))
  else:
   ix,r=prior;assert ix<=i and r['source']==a['source']
   cs=[c for c in r['components'] if c];assert len({c['identity']['pid'] for c in cs})==len(cs)
   assert all(c['role'] in ['observer','supervisor','driver'] for c in cs)
   assert s['replaced_bytes']<=sum(c['footprint_bytes'] for c in cs)//65536*65536
scans=[x for x in ev if 'scan' in x];assert all(x['scan']['dirty_bytes']==1048576 and x['scan']['native_records']==361272 and x['scan']['stub_records']==1232426 for x in scans)
assert len([x for x in ev if 'snapshot_admitted'in x])==len([x for x in ev if 'snapshot_durable'in x])==2
report=json.loads((root/'model/layout/report.json').read_text());assert report['compute']['schema']=='mycity-compute-latencies-v1';assert report['compute']['raw_bytes']==len(scans)*215*9; m=report['manifest'];full=m['r07']['logical_bytes']//65536*65536
base=m['normal_joint_budget_bytes']-m['external_owned_charge_bytes']-full-m['metadata_and_io_charge_bytes']
arenas=[x['arena'] for x in scans];grant=arenas[0]['activation']['grant_bytes'];assert grant+full+m['external_owned_charge_bytes']==m['normal_joint_budget_bytes']
assert all(a['allocation_failures']==0 and a['activation']['grant_bytes']==grant for a in arenas)
# Payload capacity equation is fixed; actual allocator charges for headers/control,
# fragmentation and bootstrap share the preexisting4MiB, not an extra grant.
peak=max(a['lifetime_peak_requested_bytes'] for a in arenas);charged_peak=max(a['arena_used_bytes'] for a in arenas)
assert peak-base<=m['metadata_and_io_charge_bytes'];assert charged_peak-base<=m['metadata_and_io_charge_bytes']
normal=m['normal_joint_budget_bytes'];peakall=max(enumerate(frames),key=lambda x:x[1]['phys_footprint_bytes']);good=[(i,f)for i,f in enumerate(frames)if f['group_capture_complete']];peakgood=max(good,key=lambda x:x[1]['phys_footprint_bytes'])
result=dict(source_sha256=m['source_sha256'],frames=len(frames),complete_frames=complete,explicit_incomplete_frames=len(frames)-complete,paused_provider_frames=paused,sampling_and_safety_never_stopped=True,actual_replacement_ack_frames=actual,reading_correlation_unknown=unknown,transitions=transitions,correlation_semantics='published sequence/owner/epoch correlated to surrounding raw frames; no simultaneous atomic-ack claim; transition intervals remain explicit',scans=len(scans),original_native_records_each=361272,original_stub_records_each=1232426,dirty_bytes_each=1048576,actual_snapshot_generations=2,metadata=dict(original_budget_bytes=m['metadata_and_io_charge_bytes'],mapping_control_capacity_bytes=m['r07_mapping_control_bytes'],actual_role_grant_bytes=grant,full_mapping_bytes=full,external_owned_bytes=m['external_owned_charge_bytes'],fixed_payload_capacity_inside_arena_bytes=base,maximum_requested_residual_within_metadata_bytes=peak-base,maximum_used_arena_residual_within_metadata_bytes=charged_peak-base,minimum_observed_arena_free_bytes=min(a['arena_free_bytes']for a in arenas),no_allocation_failures=True,scope='exact original payload-capacity equation plus actual in-arena charged observations; not kernel/native/stack proof'),normal=dict(budget_bytes=normal,all_raw_peak_frame=peakall[0],all_raw_peak_footprint_bytes=peakall[1]['phys_footprint_bytes'],complete_peak_frame=peakgood[0],complete_peak_footprint_bytes=peakgood[1]['phys_footprint_bytes'],complete_peak_over_normal_bytes=peakgood[1]['phys_footprint_bytes']-normal,accepted=False,reason='functional protocol experiment includes deliberate provider withdrawal and cold pipeline; no formal performance claim; original whole physical peak remains above normal'),eight_hour='not_run',real16gb='not_run')
(D/'raw-audit.json').write_text(json.dumps(result,indent=2)+'\n');print(json.dumps({k:result[k] for k in ['frames','complete_frames','paused_provider_frames','reading_correlation_unknown','metadata','normal']},indent=2))
