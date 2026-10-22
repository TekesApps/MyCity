from pathlib import Path
import subprocess,json,importlib.util,hashlib
root=Path(__file__).resolve().parent;exe=json.loads((root/'31-cli-build/cli-executables.json').read_text())['transport']['preserved']
spec=importlib.util.spec_from_file_location('controller',Path.cwd()/'docs/evidence/core-road-town-validation-controller/run_controller.py');mod=importlib.util.module_from_spec(spec);spec.loader.exec_module(mod)
results={}
for label,mode,requested,served,unserved in [('37-transport-normal','normal',8,8,0),('38-transport-closed','closed',25,20,5),('39-transport-congested','congested',20,20,0),('40-transport-unreachable','unreachable',5,0,5),('41-transport-normal-repeat','normal',8,8,0)]:
 with (root/(label+'-driver.log')).open('wb') as log:subprocess.run(['python3',str(root/'run-command.py'),label,'/usr/bin/time','-l',exe,mode],stdout=log,stderr=subprocess.STDOUT,check=True)
 d=root/label;r=json.loads((d/'exit.json').read_text());assert r['exit_code']==0 and r['source_unchanged'],(label,r)
 v=json.loads((d/'stdout.log').read_text());assert (v['requested_people'],v['served_people'],v['unserved_people'])==(requested,served,unserved)
 flows={f['edge_id']:f for f in v['edge_flows']};assert all(f['people']<=f['capacity_people'] for f in flows.values());assert not v['boundary_flows']
 assert sum(x['people'] for x in v['path_receipts'])==served
 assert len(v['demand_outcomes'])==1 and v['demand_outcomes'][0]['source_id']==100
 mean=v['demand_outcomes'][0]['mean_travel_seconds']
 assert mean==(sum(f['people']*f['cost_seconds'] for f in flows.values())//served if served else None)
 if mode=='closed':assert 1 not in flows and flows[2]['people']==flows[3]['people']==20 and v['unserved'][0]['reason']=='CapacityExhausted'
 elif mode=='unreachable':assert not flows and v['unserved'][0]['reason']=='NoOpenRoute'
 else:assert flows[1]['people']==served and flows[2]['people']==flows[3]['people']==0
 metrics=mod.parse_native_time_metrics((d/'stderr.log').read_text());assert metrics['status']=='captured'
 (d/'runtime-metrics.json').write_text(json.dumps(metrics,indent=2));results[label]={'mode':mode,'requested':requested,'served':served,'unserved':unserved,'mean_seconds':mean,'max_rss_bytes':metrics['max_rss_bytes']};print(json.dumps(results[label]),flush=True)
assert results['39-transport-congested']['mean_seconds']>results['37-transport-normal']['mean_seconds']
assert (root/'37-transport-normal/stdout.log').read_bytes()==(root/'41-transport-normal-repeat/stdout.log').read_bytes()
(root/'transport-cli-summary.json').write_text(json.dumps({'cases':results,'repeat_stdout_identical':True,'validator_sha256':hashlib.sha256(Path(__file__).read_bytes()).hexdigest(),'scope':'actual bounded CLI scenarios; not full-map performance or 16GB acceptance'},indent=2))
