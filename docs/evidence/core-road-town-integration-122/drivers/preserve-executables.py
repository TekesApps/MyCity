from pathlib import Path
import sys,re,hashlib,subprocess,json
root=Path(__file__).resolve().parent; d=root/sys.argv[1];rows=[]
for line in (d/'stderr.log').read_text().splitlines():
 m=re.search(r'(?:Running|Executable) .+ \((.+)\)$',line)
 if not m:continue
 p=Path(m.group(1));assert p.is_relative_to(Path('/Volumes/HP P900/dev/MyCity/target/release')) and p.is_file()
 h=hashlib.sha256(p.read_bytes()).hexdigest();out=root/'executables'/h;out.mkdir(exist_ok=True);dest=out/p.name
 if not dest.exists():subprocess.run(['/bin/cp','-c',str(p),str(dest)],check=True)
 assert hashlib.sha256(dest.read_bytes()).hexdigest()==h
 rows.append({'built_path':str(p),'sha256':h,'preserved':str(dest)})
assert rows,'no executable paths in cargo log'
(d/'executables.json').write_text(json.dumps(rows,indent=2));print('preserved='+str(len(rows)))
