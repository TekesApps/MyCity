from pathlib import Path
import json,subprocess,hashlib,sys
root=Path(__file__).resolve().parent
meta=json.loads((root/'integer-integrated-examples.json').read_text())
bins={x['name']:Path(x['binary']) for x in meta['binaries']}
for row in meta['binaries']:assert hashlib.sha256(Path(row['binary']).read_bytes()).hexdigest()==row['binary_sha256']
out=root/'108-canonical-integrated-clis'/'cases';out.mkdir(parents=True,exist_ok=False)
cases=[('normal',[]),('held',['--hold-road-inspection']),('generator-failure',['--generator-failure']),('construction-utilities',['--construction-utilities']),('construction-held',['--construction-utilities','--hold-road-inspection'])]
rows=[]
for name,args in cases:
 d=out/name;d.mkdir();cmd=[str(bins['town_lifelines']),'--store-dir',str(d/'world'),*args]
 with (d/'stdout.log').open('wb') as o,(d/'stderr.log').open('wb') as e:r=subprocess.run(cmd,stdout=o,stderr=e)
 rows.append({'case':name,'argv':cmd,'exit_code':r.returncode})
d=out/'employment';d.mkdir();cmd=[str(bins['town_employment']),'--switch-day=10']
with (d/'stdout.log').open('wb') as o,(d/'stderr.log').open('wb') as e:r=subprocess.run(cmd,stdout=o,stderr=e)
rows.append({'case':'employment','argv':cmd,'exit_code':r.returncode})
(out/'results.json').write_text(json.dumps(rows,indent=2));print(json.dumps(rows,indent=2))
sys.exit(0 if all(x['exit_code']==0 for x in rows) else 1)
