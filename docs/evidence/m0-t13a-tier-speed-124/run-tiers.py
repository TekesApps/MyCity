from pathlib import Path
import hashlib,json,os,subprocess,time
root=Path(__file__).resolve().parent
binary=Path('/Volumes/HP P900/dev/MyCity/target/release/mycity-headless')
build=json.loads((root/'05-cli-clock-build/exit.json').read_text());assert build['exit_code']==0 and build['source_unchanged']
hash=hashlib.sha256(binary.read_bytes()).hexdigest();dest=root/'executables'/hash/binary.name;dest.parent.mkdir(exist_ok=True)
if not dest.exists():subprocess.run(['/bin/cp','-c',str(binary),str(dest)],check=True)
assert hashlib.sha256(dest.read_bytes()).hexdigest()==hash
info=json.loads((root/'binary.json').read_text());info.update(binary=str(dest),binary_sha256=hash,source_zip_sha256=hashlib.sha256((root/'05-cli-clock-build/source.zip').read_bytes()).hexdigest(),source_manifest_sha256=hashlib.sha256((root/'05-cli-clock-build/source-manifest.json').read_bytes()).hexdigest())
(root/'binary-final.json').write_text(json.dumps(info,indent=2))
for number,tier in [(6,'small'),(7,'medium'),(8,'full')]:
 label=f'{number:02}-{tier}'; observed=root/(label+'-observed'); command_label=label+'-functional'
 processes=subprocess.check_output(['ps','-axo','pid=,ppid=,comm='],text=True)
 (root/(label+'-processes-before.txt')).write_text(processes)
 # No own build or other model can overlap. External projects are recorded,
 # and this run never grants a formal speed/resource performance pass.
 for line in processes.splitlines():
  fields=line.strip().split(None,2)
  if len(fields)==3:assert Path(fields[2]).name!='mycity-headless',line
 argv=['python3',str(root/'run-command.py'),command_label,str(dest),'native-observe',str(observed),'--',str(dest),'speed-run',str(observed/'model'),tier,'unpaced']
 subprocess.run(argv,check=True)
 result=json.loads((root/command_label/'exit.json').read_text())
 assert result['exit_code']==0 and result['source_unchanged'],result
 assert hashlib.sha256(dest.read_bytes()).hexdigest()==hash
 print(tier+' complete',flush=True)
