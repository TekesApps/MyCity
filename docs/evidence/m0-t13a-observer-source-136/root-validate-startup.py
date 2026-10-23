from pathlib import Path
import sys,json,subprocess,hashlib,time,os
B=Path(__file__).resolve().parent
D=B/'root-validation-02';D.mkdir(exist_ok=False)
sys.path.insert(0,str(B));import run
run.verify_support_manifest()
src=json.loads((B/'source.json').read_text());source=src['fixed_source_sha256'];exe=src['fixed_binary'];rows=[]
try:
 for name,flags,creates in [('missing-key-argument',[],False),('mismatch-source',['--fixed-source-sha256','0'*64],False),('actual-rust-report-fault',['--fixed-source-sha256',source],True)]:
  root=D/name;cmd=[sys.executable,str(B/'observe.py'),str(root),*flags,'--',exe,'layout-r07-session',str(root/'model'),'--report-fault']
  t=time.monotonic();p=subprocess.run(cmd,capture_output=True,text=True,env={**os.environ,'PYTHONDONTWRITEBYTECODE':'1'})
  (D/(name+'.stdout')).write_text(p.stdout);(D/(name+'.stderr')).write_text(p.stderr)
  row={'case':name,'argv':cmd,'exit_code':p.returncode,'wall_seconds':time.monotonic()-t,'root_exists':root.exists(),'expected_root_exists':creates};rows.append(row)
  (D/'actual-startup-cases.json').write_text(json.dumps(rows,indent=2)+'\n')
  assert p.returncode!=0 and root.exists()==creates
  if creates:
   r=json.loads((root/'observer-report.json').read_text());row.update({k:r[k] for k in ['status','reason','model_exit_code','frames_observed']})
   assert r['reason']=='clock peer exited' and r['model_exit_code']==2 and r['frames_observed']==0
   assert (root/'model.stderr').read_text().strip()=='metrics_unknown_class'
  else:assert not (root/'model').exists()
 out={'status':'passed','cases':rows,'support_manifest_sha256':hashlib.sha256((B/'support-manifest.json').read_bytes()).hexdigest(),'scope':'negative observer startup and actual frozen Rust fault exit; no smoke, timing, or clock success credit'}
except BaseException as error:
 out={'status':'failed_validation','cases':rows,'error':repr(error)}
 (D/'actual-startup-validation.json').write_text(json.dumps(out,indent=2)+'\n');raise
(D/'actual-startup-validation.json').write_text(json.dumps(out,indent=2)+'\n');print(json.dumps(out,indent=2))
