from pathlib import Path
import subprocess,json,hashlib,sys
r=Path(__file__).resolve().parent;m=json.loads((r/'employment-lifelines-freight-binary.json').read_text());exe=m['binary'];out=r/'122-employment-lifelines-rejections';rows=[]
for n,args in [('conflicting-employment',['--employment-control','--employment-change']),('held-employment',['--employment-change','--hold-road-inspection']),('failed-generator-employment',['--employment-change','--generator-failure'])]:
 d=out/(n+'-must-not-exist');cmd=[exe,'--store-dir',str(d),*args];p=subprocess.run(cmd,capture_output=True,text=True);(out/(n+'.stdout')).write_text(p.stdout);(out/(n+'.stderr')).write_text(p.stderr);assert p.returncode==1 and not d.exists(),n;rows.append({'case':n,'exit_code':p.returncode,'store_created':False,'argv':cmd})
world=r/'120-employment-lifelines-freight-cases/cases/employment-change/world'
def state():return {str(p.relative_to(world)):hashlib.sha256(p.read_bytes()).hexdigest() for p in world.rglob('*') if p.is_file()}
before=state();cmd=[exe,'--employment-change','--store-dir',str(world)];p=subprocess.run(cmd,capture_output=True,text=True);(out/'reuse.stdout').write_text(p.stdout);(out/'reuse.stderr').write_text(p.stderr);assert p.returncode==1 and state()==before
rows.append({'case':'existing-store','exit_code':p.returncode,'world_file_hashes_unchanged':True,'world_files':len(before),'argv':cmd});(out/'results.json').write_text(json.dumps(rows,indent=2));print(json.dumps(rows,indent=2))
