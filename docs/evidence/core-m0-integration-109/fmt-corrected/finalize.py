import hashlib,json,os,struct,re,subprocess
from pathlib import Path
R=Path('/Users/frankqin/dev/MyCity');os.chdir(R);D=R/'docs/evidence/core-m0-integration-109/fmt-corrected';f=json.loads((D/'freeze.json').read_text());h=hashlib.sha256()
files=sorted(['.cargo/config.toml','Cargo.toml','Cargo.lock','rust-toolchain.toml']+[p.relative_to(R).as_posix() for b in ['crates','fixtures'] for p in (R/b).rglob('*') if p.is_file()])
for n in files:
 b=(R/n).read_bytes();name=n.encode();h.update(struct.pack('<Q',len(name)));h.update(name);h.update(struct.pack('<Q',len(b)));h.update(b)
assert h.hexdigest()==f['source_sha256']; source=h.hexdigest();E=Path('/Volumes/HP P900/dev/MyCity/acceptance-binaries')/source;E.mkdir(parents=True,exist_ok=False);L=R/'work/candidate-binaries/core-m0-integration'/source;L.parent.mkdir(parents=True,exist_ok=True);L.symlink_to(E,target_is_directory=True)
paths={R/'target/release/mycity-headless',R/'target/release/examples/town_market',R/'target/release/examples/market_access'}
for p in D.glob('*.stderr'):
 for n in re.findall(r'Running [^\n]*\((target/[^)]+)\)',p.read_text()):paths.add(R/n)
rows=[]
for p in sorted(paths):
 dst=E/p.name;os.link(p,dst);sha=hashlib.sha256(dst.read_bytes()).hexdigest();rows.append(dict(source=str(p),fixed=str(L/p.name),physical_fixed=str(dst),sha256=sha,same_inode=p.stat().st_ino==dst.stat().st_ino))
(D/'fixed-executables.json').write_text(json.dumps(dict(source_sha256=source,storage='external disk; canonical work alias; hardlinks add no duplicate payload',executables=rows),indent=2)+'\n')
results=[]
for name,args in [('town-market',[])]+[(n,[n]) for n in ['normal','road_closed','bottleneck','capacity_expanded','cross_city','unrelated_cross_city']]:
 cmd=[str(L/('town_market' if name=='town-market' else 'market_access'))]+args
 with (D/(name+'.cli.stdout')).open('x') as out,(D/(name+'.cli.stderr')).open('x') as err:q=subprocess.run(cmd,stdout=out,stderr=err)
 results.append(dict(scenario=name,command=cmd,exit=q.returncode,source_sha256=source));assert q.returncode==0,(name,q.returncode)
(D/'cli-results.json').write_text(json.dumps(results,indent=2)+'\n');print(json.dumps(dict(fixed=len(rows),actual_cli_passed=len(results),source_sha256=source)))
