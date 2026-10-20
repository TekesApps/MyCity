#!/usr/bin/env python3
"""Actual Rust full CPU proof plus synthetic image-handoff counterexamples.
No throughput/steady/pressure/cold Store gate is signed by this script.
"""
import hashlib,json,os,signal,subprocess,sys,time,zipfile
from pathlib import Path
D=Path(__file__).resolve().parent;R=D.parents[2]
B=R/'target/release/mycity-headless';W=R/'work'/(sys.argv[1] if len(sys.argv)>1 else 't13a3g-model-tasks-functional');W.mkdir()
M=W/'model';events=[]
sha=lambda b:hashlib.sha256(b).hexdigest()
def save(p,v):p.write_text(json.dumps(v,indent=2)+'\n')
def pressure():return subprocess.check_output(['/usr/sbin/sysctl','-n','kern.memorystatus_vm_pressure_level'],text=True).strip()
def call(name,args,ok=True):
 start=time.monotonic();before=pressure()
 with (W/(name+'.stdout')).open('xb') as out,(W/(name+'.stderr')).open('xb') as error:
  child=subprocess.Popen([str(B),*map(str,args)],stdout=out,stderr=error,start_new_session=True,env={**os.environ,'MYCITY_HEAP_LOG':str(W/'heaps.jsonl')})
  try:code=child.wait(timeout=120)
  except subprocess.TimeoutExpired:
   os.killpg(child.pid,signal.SIGTERM);child.wait(timeout=5);raise
 event={'name':name,'pid':child.pid,'exit':code,'wall_seconds':time.monotonic()-start,'pressure_before':before,'pressure_after':pressure()};events.append(event);save(W/'events.json',events)
 assert (code==0)==ok,event
 return (W/(name+'.stderr')).read_text()
call('reference-full',['scale-model-reference-probe',M,'full'])
new_targets=[json.loads(s) for s in (M/'cpu-targets.jsonl').read_text().splitlines()]
assert len(new_targets)==723
legacy=R/'docs/evidence/m0-t13a-process-instances/t13a3c-instances-01/artifacts.zip'
with zipfile.ZipFile(legacy) as z:
 old_targets=[json.loads(s) for s in z.read('model/cpu-targets.jsonl').decode().splitlines()]
 assert new_targets==old_targets
 assert (M/'cpu-final-engine.json').read_bytes()==z.read('model/cpu-final-engine.json')
 old=json.loads(z.read('model/cpu-report.json'));new=json.loads((M/'cpu-report.json').read_text())
 for field in ('source_sha256','work_wall_seconds'):old.pop(field);new.pop(field)
 assert old==new
base=json.loads((M/'reference/request.json').read_text())
def task(mode):
 directory=M/mode.replace('_','-');directory.mkdir()
 save(directory/'request.json',{**base,'mode':mode})
 return directory
# This deliberately constructed chain tests only the image verifier and Compare.
# It is NOT a Store save or real cold reopen; the actual checkpoint bytes are CPU output.
world=M/'world';world.mkdir();image=(M/'cpu-final-engine.json').read_bytes()
def install_image(data):
 (world/'world-1.json').write_bytes(data)
 index=json.dumps({'current':{'generation':1,'sequence':123,'bytes':len(data),'sha256':sha(data)}}).encode();(world/'world-index-1.json').write_bytes(index)
 root=json.dumps({'checkpoint_layout':2,'sequence':123,'checkpoint_v2':{'generation':1,'sha256':sha(index)}}).encode();(world/'root-0.json').write_bytes(root)
 save(world/'CURRENT.json',{'slot':0,'root_sha256':sha(root)})
install_image(image)
before=task('before_reopen');call('before-synthetic-image',['scale-model-task',before])
compare=task('compare')
install_image(image+b' ')
assert 'scale_sim_saved_image_changed' in call('changed-image-rejected',['scale-model-task',compare],False)
assert not (compare/'result.json').exists()
compare.rename(M/'compare-changed-image-failed');compare=task('compare')
install_image(image)
original=(M/'reference/cpu-final-engine.json').read_bytes();bad=bytearray(original);bad[0]^=1
(M/'reference/cpu-final-engine.json').write_bytes(bad)
assert 'model_task_artifact_integrity' in call('bad-reference-rejected',['scale-model-task',compare],False)
(M/'reference/cpu-final-engine.json').write_bytes(original)
compare.rename(M/'compare-bad-reference-failed');compare=task('compare')
call('compare-synthetic-image',['scale-model-task',compare])
result=json.loads((compare/'result.json').read_text());assert result['business_sha256']==new['business_sha256']
assert 'model_task_result_exists' in call('old-result-rejected',['scale-model-task',compare],False)
# Result binding is checked even before expensive model work.
request=before/'request.json';original=request.read_bytes();payload=json.loads(original);payload['oracle_sha256']='0'*64;save(request,payload)
assert 'model_task_request_identity' in call('wrong-oracle-rejected',['scale-model-task',before],False)
request.write_bytes(original)
records=[json.loads(s) for s in (W/'heaps.jsonl').read_text().splitlines()]
assert len(records)==len(events)+1
identities={(r['pid'],r['instance_unix_ns']) for r in records};assert len(identities)==len(records)
assert sum(r['role']=='scale-model-reference-probe' and r['success'] for r in records)==1
assert sum(r['role']=='scale-model-task' and r['success'] for r in records)==3
save(W/'verification.json',{'full_CPU_723_targets_match_v42':True,'full_CPU_engine_bytes_match_v42':True,'all_CPU_report_fields_except_source_and_wall_match_v42':True,'source_sha256':base['source_sha256'],'binary_sha256':sha(B.read_bytes()),'legacy_archive_sha256':sha(legacy.read_bytes()),'helper_instance_records':len(records),'synthetic_image_chain_comparison':'passed_synthetic_only_not_Store_cold_reopen','real_Sim_full_30_days':'not_run_this_source','diagnostic_8MiB_gate':'not_accepted','external_64MiB_gate':'not_accepted','performance_gate':'not_run','eight_hour_gate':'not_run','memory_16gb_gate':'not_run','full_M0':'not_accepted'})
print((W/'verification.json').read_text())
