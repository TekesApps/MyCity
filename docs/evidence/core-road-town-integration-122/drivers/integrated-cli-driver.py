from pathlib import Path
import hashlib,json,subprocess,re
r=Path(__file__).resolve().parent
names=['town_transport','town_suspend','payroll_exit','construction','unfinished_clearance','town_utility','town_market']
executables={}
for name in names:
 p=Path('/Volumes/HP P900/dev/MyCity/target/release/examples')/name
 h=hashlib.sha256(p.read_bytes()).hexdigest();d=r/'executables'/h;d.mkdir(exist_ok=True);e=d/name
 if not e.exists():subprocess.run(['/bin/cp','-c',str(p),str(e)],check=True)
 assert hashlib.sha256(e.read_bytes()).hexdigest()==h
 executables[name]={'sha256':h,'preserved':str(e)}
(r/'49-cli-rebuild'/'cli-executables.json').write_text(json.dumps(executables,indent=2))
cases=[]
for i,mode in enumerate(['normal','closed','bottleneck','expanded','cross-boundary','road-effect','road-suspend','road-28','road-withdraw','road-clearance'],50):
 label=f'{i}-town-transport-{mode}';cases.append((label,'town_transport',[mode,str(r/label/'world')]))
for i,name,options in [(60,'town_suspend',[]),(61,'town_suspend',['--loss']),(62,'payroll_exit',[])]:
 label=f'{i}-{name}'+('-loss' if options else '');cases.append((label,name,[str(r/label/'world'),*options]))
for i,name,options in [(63,'construction',[]),(64,'unfinished_clearance',[]),(65,'town_utility',[]),(66,'town_utility',['--generator-failure'])]:
 cases.append((f'{i}-{name}'+('-failure' if options else ''),name,options))
results=[]
for label,name,args in cases:
 with (r/(label+'-driver.log')).open('x') as log:
  subprocess.run(['python3',str(r/'run-command.py'),label,'/usr/bin/time','-l',executables[name]['preserved'],*args],stdout=log,stderr=subprocess.STDOUT,check=True)
 d=r/label;v=json.loads((d/'exit.json').read_text());err=(d/'stderr.log').read_text()
 metrics={'max_rss_bytes':int(re.search(r'(\d+)\s+maximum resident set size',err).group(1)),'storage':'HP P900 external USB/APFS SSD','scope':'bounded functional CLI; not full-map or 16GB acceptance','executable':executables[name]}
 (d/'runtime-metrics.json').write_text(json.dumps(metrics,indent=2))
 results.append({'label':label,**v,**metrics});(r/'integrated-cli-summary.json').write_text(json.dumps(results,indent=2))
 print(label, v,flush=True)
 if v['exit_code']!=0 or not v['source_unchanged']:raise SystemExit('FAILED '+label)
