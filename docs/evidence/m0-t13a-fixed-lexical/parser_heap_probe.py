"""Deterministic isolated parser diagnostics. No peak reset, no normal-pool claim.
Run only after the frozen regression controller is terminal. Shared deps reused.
"""
from pathlib import Path
import hashlib,json,os,subprocess,time
D=Path(__file__).resolve().parent;R=D.parents[2]
os.chdir(R)
def sha(p):return hashlib.sha256(Path(p).read_bytes()).hexdigest()
def save(n,v): (D/n).write_text(json.dumps(v,indent=2)+'\n')
assert (D/'metadata-terminal.json').exists(),'wait for actual frozen business/controller terminal'
root=R/'work/t13a-lexical-parser-inputs';root.mkdir()
fixed=R/'work/candidate-binaries/role-workspace-lexical/parser-heap'
assert not fixed.exists()
# These are the linked artifacts reported by the exact-source Cargo build, not guessed stale rlibs.
names=['serde','serde_json','peak_alloc'];libs={}
for line in (D/'metadata-build-final.stdout').read_text().splitlines():
 x=json.loads(line)
 if x.get('reason')=='compiler-artifact' and x['target']['name'] in names:
  libs[x['target']['name']]=next(n for n in x['filenames'] if n.endswith('.rlib'))
assert set(libs)==set(names)
cmd=['rustc','--edition=2021','-O',str(D/'parser-heap.rs'),'-L','dependency='+str(R/'target/release/deps')]
for n,p in libs.items():cmd+=['--extern',n+'='+p]
cmd+=['-o',str(fixed)]
with (D/'parser-heap-compile.stdout').open('x') as o,(D/'parser-heap-compile.stderr').open('x') as e:subprocess.run(cmd,stdout=o,stderr=e,check=True)
freeze=json.loads((D/'lexical-freeze.json').read_text())
manifest=[];out=[]
limit=16*1024*1024

def probe(name,kind,data,expected):
 p=root/(name+'.json');p.write_bytes(data)
 st=p.stat();manifest.append(dict(path=str(p.relative_to(R)),bytes=len(data),sha256=sha(p),dev=st.st_dev,inode=st.st_ino,mtime_ns=st.st_mtime_ns,allocated_bytes=st.st_blocks*512,hardlinks=st.st_nlink,reason='Generated synthetic parser input, not business save; all measured processes terminal before retirement',retained_evidence='docs/evidence/m0-t13a-fixed-lexical/parser-heap-results.json and parser_heap_probe.py; no direct recovery claim'))
 for mode in ['old','fixed']:
  run=[str(fixed),mode,kind,str(p)]
  with (D/(name+'-'+mode+'.stdout')).open('x') as o,(D/(name+'-'+mode+'.stderr')).open('x') as e:subprocess.run(run,stdout=o,stderr=e,check=True)
  fact=json.loads((D/(name+'-'+mode+'.stdout')).read_text())
  assert fact['accepted']==expected,(name,mode,fact)
  if mode=='fixed':
   assert fact['parser_capacity']==1024*1024 and fact['backing_unchanged']
   assert fact['after_lifetime_peak']<=fact['before_lifetime_peak']+4096,(name,fact)
  fact['case']=name;out.append(fact)
 # No accumulated generator data lives in measured child processes; each old/new starts fresh.
 print(name,'old/fixed acceptance matched',flush=True)

probe('small-profile','profile',b'{"flag":true}',True)
for size,label in [(64*1024,'64k'),(limit,'16m')]:
 prefix=b'{"';suffix=b'":0,"flag":true}'
 n=(size-len(prefix)-len(suffix))//6
 probe('escaped-key-'+label,'profile',prefix+b'\\u0061'*n+suffix,True)
 prefix=b'{"flag":"';suffix=b'"}'
 probe('wrong-bool-'+label,'profile',prefix+b'\\n'*((size-len(prefix)-len(suffix))//2)+suffix,False)
 prefix=b'"';suffix=b'"'
 probe('escaped-value-'+label,'content',prefix+b'\\n'*((size-2)//2)+suffix,True)
 probe('deep-ignore-'+label,'ignore',b'['*(size//2)+b']'*(size//2),True)
probe('malformed-overstack-16m','ignore',b'['*limit,False)
probe('late-invalid-unicode-key','profile',b'{"'+b'\\u0061'*100000+b'\\ud800":0,"flag":true}',False)
save('parser-input-manifest.json',manifest)
save('parser-heap-results.json',dict(source_sha256=freeze['source_sha256'],helper_sha256=sha(fixed),helper_source_sha256=sha(D/'parser-heap.rs'),linked_libraries={n:dict(path=p,sha256=sha(p)) for n,p in libs.items()},results=out,scope='Isolated exact parser code plus pinned serde_json scalar/grammar comparison; input fs::read and retained scratch included in lifetime requested heap; no reset. Content comparison uses serde_json::Value scalar/grammar oracle, not a complete Store workload or exact private serde Content layout. No physical memory, synchronous processes, normal grant or formal speed acceptance. All children terminal; generated inputs may be retired after occupancy/dependency recheck.'))
print('Parser probes terminal0',flush=True)
