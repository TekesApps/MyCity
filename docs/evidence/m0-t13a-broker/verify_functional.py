#!/usr/bin/env python3
"""Bind the extra Python launcher and unchanged Rust binary separately."""
import hashlib,json,sys
from pathlib import Path
R=Path(sys.argv[1]);D=Path(__file__).resolve().parent;load=lambda p:json.loads(p.read_text());sha=lambda p:hashlib.sha256(p.read_bytes()).hexdigest()
info=load(D/'source.json');o=load(R/'observer-report.json');m=load(R/'model/manifest.json');v=load(R/'model/runner-report.json');pool=load(R/'model/normal-pool/report.json');business=load(R/'model/business/report.json')
assert o['status']=='passed_measurement_wrapper' and o['binary_unchanged'] and o['observer_source_unchanged']
assert o['binary_sha256']==m['runner_sha256']==v['runner_sha256']==sha(D/'pool_full.py')
assert o['observer_source_sha256']==info['observer_sha256']
assert info['binary_sha256']==m['rust_binary_sha256']==v['rust_binary_sha256']
assert info['source_sha256']==m['source_sha256']==v['source_sha256']==pool['source_sha256']==business['source_sha256']
assert v['status']=='completed_functional_only' and v['reason'] is None and v['Rust_binary_unchanged'] and v['runner_unchanged']
assert v['hot_trace']['charged_hot_bytes']==v['hot_trace']['peak_bytes']+v['hot_trace']['trace_metadata_bytes']<=v['hot_trace']['cap_bytes']==1024**2
assert pool['ready']['pid']==v['pool_pid'] and pool['ready']['fault_injection'] is None and pool['balanced_after_owned_wait']
assert pool['summary']['grants']==pool['summary']['returns']==pool['summary']['fill_acknowledgements']==724
assert business['completed_hour_targets']==720 and business['verified_drive_targets']==723 and business['actual_worker_windows']==724 and business['saved_sequence']==3011
assert business['complete_final_business_and_air']=='passed' and business['cold_complete_image_unchanged'] and business['attempt_failures']==0
print(json.dumps({'status':'same_Rust_full_business_plus_pool_functional_branch_bound','source_sha256':info['source_sha256'],'Rust_binary_sha256':info['binary_sha256'],'launcher_script_sha256':m['runner_sha256'],'observer_entry_binary_sha_is_launcher_script_not_Rust':True,'runner_hot_trace':v['hot_trace'],'observer_actual_wall_seconds':o['actual_wall_seconds'],'business_work_seconds':business['business_work_seconds'] if 'business_work_seconds' in business else None,'all_children_waited_successfully':True,'L1':'absent_not_joint_pressure','original_joint_attempt':'failed_preserved','shared_8MiB':'not_accepted_extra_interpreter_startup_native_unclosed','exact_normal_owned':'not_accepted','performance':'not_accepted','formal_steady':'not_run','four_speeds':'not_run','eight_hour':'not_run','real_16GB':'not_run'},indent=2))
