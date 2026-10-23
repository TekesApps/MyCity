"""Post-run identity/envelope analysis; never asserts instantaneous open-time alignment."""
import argparse,json,math,statistics
from pathlib import Path
p=argparse.ArgumentParser();p.add_argument('run',type=Path);p.add_argument('pin',type=Path);a=p.parse_args()
pin=json.loads(a.pin.read_text()); groups={};births={};observer=None;minutes={};coverage={'observer_present':0,'observer_missing':0,'complete_group_frames':0,'frames':0};global_peak=None
for line in (a.run/'host-samples.jsonl').open():
 f=json.loads(line);coverage['frames']+=1
 if f['group_capture_complete']:coverage['complete_group_frames']+=1
 for m in f['members']:
  if m[4]=='observer':
   found=(m[0],m[2]);assert observer is None or observer==found;observer=found
 rows={(x[0],x[1]):x[2] for x in f['member_footprints_pid_birth_bytes']}
 for key,value in rows.items():
  births.setdefault(key[0],set()).add(key[1]);g=groups.setdefault(key,{'min':value,'max':value,'samples':0});g['samples']+=1;g['min']=min(g['min'],value);g['max']=max(g['max'],value)
 if observer in rows:
  coverage['observer_present']+=1;minutes.setdefault(int(f['clock_end_ns']//60_000_000_000),[]).append(rows[observer])
 else:coverage['observer_missing']+=1
 if f['group_capture_complete'] and (global_peak is None or f['phys_footprint_bytes']>global_peak['phys_footprint_bytes']):
  global_peak={'clock_begin_ns':f['clock_begin_ns'],'clock_end_ns':f['clock_end_ns'],'phys_footprint_bytes':f['phys_footprint_bytes'],'components':f['member_footprints_pid_birth_bytes']}
summary=[]
for minute,vals in sorted(minutes.items()):
 vals.sort();summary.append({'minute':minute,'samples':len(vals),'median_bytes':statistics.median(vals),'p95_bytes':vals[max(0,math.ceil(len(vals)*.95)-1)],'max_bytes':max(vals)})
report=json.loads((a.run/'observer-report.json').read_text());assert report['source_sha256']==pin['source_sha256']
receipt_rows=[]
for transcript in (a.run/'model/business').rglob('transcript.jsonl'):
 for line in transcript.open():
  row=json.loads(line);r=row.get('data',{}).get('sim_open_memory_diagnostic')
  if r is None:continue
  pid=r['sim_pid'];matches=sorted(births.get(pid,set()));entry={'sim_pid':pid,'app_instance_unix_ns':r['sim_birth_unix_ns'],'operation':r['operation'],'native_birth_candidates':matches,'native_clock_alignment':'not_available; no instantaneous-open footprint claim'}
  if len(matches)==1:
   entry['observed_lifecycle_footprint_envelope_bytes']=groups[(pid,matches[0])];entry['native_parent_link']='see exact host members and registration rows; PID uniqueness alone is insufficient'
  else:entry['identity_status']='missing_or_ambiguous'
  receipt_rows.append(entry)
print(json.dumps({'source_sha256':pin['source_sha256'],'observer_native_identity':observer,'coverage':coverage,'observer_per_minute':summary,'raw_observed_group_peak':global_peak,'sim_lifecycle_receipts':receipt_rows,'scope':'post-run observed native identity/envelopes; app and native clocks are not aligned here','normal_memory_acceptance':'not_accepted','eight_hour_gate':'not_run','actual16GB':'not_run'},indent=2))
