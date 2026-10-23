from pathlib import Path
import json,subprocess,hashlib,sys
r=Path(__file__).resolve().parent;m=json.loads((r/'employment-lifelines-freight-binary.json').read_text());exe=Path(m['binary']);assert hashlib.sha256(exe.read_bytes()).hexdigest()==m['binary_sha256']
root=r/'120-employment-lifelines-freight-cases'/'cases';root.mkdir(parents=True,exist_ok=False)
cases=[('normal',[]),('held',['--hold-road-inspection']),('generator-failure',['--generator-failure']),('construction-utilities',['--construction-utilities']),('construction-held',['--construction-utilities','--hold-road-inspection']),('employment-control',['--employment-control']),('employment-change',['--employment-change'])]
rows=[]
for name,args in cases:
 d=root/name;d.mkdir();cmd=[str(exe),'--store-dir',str(d/'world'),*args]
 with (d/'stdout.log').open('wb') as o,(d/'stderr.log').open('wb') as e:p=subprocess.run(cmd,stdout=o,stderr=e)
 rows.append({'case':name,'argv':cmd,'exit_code':p.returncode});print(name+':'+str(p.returncode),flush=True)
(root/'results.json').write_text(json.dumps(rows,indent=2));sys.exit(0 if all(x['exit_code']==0 for x in rows) else 1)
