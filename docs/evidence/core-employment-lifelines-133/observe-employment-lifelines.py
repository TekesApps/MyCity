from pathlib import Path
import subprocess,json,re,hashlib
r=Path(__file__).resolve().parent;m=json.loads((r/'employment-lifelines-freight-binary.json').read_text());exe=m['binary'];out=r/'123-employment-lifelines-native-observation';rows=[]
for name in ['employment-control','employment-change']:
 d=out/name;d.mkdir();cmd=['/usr/bin/time','-l',exe,'--store-dir',str(d/'world'),'--'+name]
 with (d/'stdout.log').open('wb') as o,(d/'stderr.log').open('wb') as e:p=subprocess.run(cmd,stdout=o,stderr=e)
 assert p.returncode==0
 text=(d/'stdout.log').read_text();old=(r/'120-employment-lifelines-freight-cases/cases'/name/'stdout.log').read_text();state=re.search(r' state_hash=([a-f0-9]{64})',text).group(1);assert state==re.search(r' state_hash=([a-f0-9]{64})',old).group(1)
 raw=(d/'stderr.log').read_text();rss=re.search(r'(\d+)\s+maximum resident set size',raw);foot=re.search(r'(\d+)\s+peak memory footprint',raw)
 rows.append({'case':name,'argv':cmd,'exit_code':p.returncode,'state_hash_matches_120':state,'max_rss_bytes':int(rss.group(1)) if rss else None,'peak_footprint_bytes':int(foot.group(1)) if foot else None,'time_stderr':'stderr.log'})
v={'runs':rows,'scope':'two small CLI whole-process observations and deterministic repeat, no clean admission, no incremental allocation or 16GB/M0 performance pass','storage':'external HP P900 USB/APFS SSD','source_sha256':m['source_sha256'],'binary_sha256':m['binary_sha256']};(out/'observation.json').write_text(json.dumps(v,indent=2));print(json.dumps(v,indent=2))
