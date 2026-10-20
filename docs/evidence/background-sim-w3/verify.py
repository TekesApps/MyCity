#!/usr/bin/env python3
"""W3 frozen process evidence: actual transcripts, committed roots and whole air state."""
import argparse, hashlib, json, re, struct, subprocess, tempfile, zipfile
from pathlib import Path
HERE=Path(__file__).resolve().parent
ROOT=HERE.parents[2]
SOURCE='b31269a45648b77f51c8d5e9102900b1137921dbdcf645525e243974371dbeb6'
def require(ok,reason):
    if not ok: raise SystemExit(reason)
def digest(data):return hashlib.sha256(data).hexdigest()
def source_hash():
    names=['.cargo/config.toml','Cargo.toml','Cargo.lock','rust-toolchain.toml']+[p.relative_to(ROOT).as_posix() for folder in ['crates','fixtures'] for p in (ROOT/folder).rglob('*') if p.is_file()]
    h=hashlib.sha256()
    for name in sorted(names):
        b=name.encode();data=(ROOT/name).read_bytes();h.update(struct.pack('<Q',len(b)));h.update(b);h.update(struct.pack('<Q',len(data)));h.update(data)
    return h.hexdigest()
def report_check(report):
    require(report['source_sha256']==SOURCE,'source mismatch')
    cases=report['cases'];require({c['case'] for c in cases}=={'baseline','worker_sources','worker_cells','sim_worker_cells','sim_leased','sim_asset','sim_committed_reply','cancel_retry'},'missing process case')
    require(len({c['business_without_revision_sha256'] for c in cases})==1,'business differs')
    require(len({c['complete_air_state_sha256'] for c in cases})==1,'air differs')
    for c in cases:
        state=c['status'];require(state['background']['time']==30 and state['background']['version']==1,'air task incomplete')
        require(c['receipt']['record']['operation_id']==0xe000000000000001,'noncanonical admission receipt')
        require(c['receipt']['replayed']==(c['case']=='sim_committed_reply'),'lost reply not recovered')
        require(c['persistent_reopen_verified'],'missing reopen')
        capacity=state['capacity'];require(capacity['worker_cache_charged_bytes']==6*1024*1024 and capacity['worker_cache_actual_bytes']<=6*1024*1024,'cache quota')
    for field in ['whole_process_group_measurement','endurance_1800_seconds','memory_16gb_gate']:
        require(report[field]=='not_run','unverified scope claimed')
def main():
    parser=argparse.ArgumentParser();parser.add_argument('--rerun',action='store_true');args=parser.parse_args()
    require(source_hash()==SOURCE,'use the frozen source/tag for this verifier')
    report=json.loads((HERE/'probe.json').read_bytes());report_check(report)
    gates=json.loads((HERE/'gates.json').read_bytes())
    require({g['gate'] for g in gates}=={'fmt','clippy','release-tests'} and all(g['exit_code']==0 for g in gates),'release gates incomplete')
    log=(HERE/'release-tests.txt').read_text()
    require('test result: FAILED' not in log,'failed release tests')
    for test in ['actual_worker_sigkill_in_sources_and_cells_retries_in_same_sim_with_new_generation','actual_sim_sigkill_at_lease_uncommitted_asset_and_committed_reply_recovers','three_real_cancelled_workers_exhaust_attempts_without_erasing_required_job','internal_control_namespace_cannot_be_preoccupied_by_execute','worker_cache_charge_survives_missing_files_and_moves_out_of_task_reserve_once','canonical_accept_rejections_do_not_create_wal_or_cold_failed_receipts','normal_disk_exhaustion_preserves_background_and_timeline_funding']:
        require(f'test {test} ... ok' in log,'missing test '+test)
    manifest=json.loads((HERE/'artifact-manifest.json').read_bytes())
    reference_world=None;case_checks=[]
    with zipfile.ZipFile(HERE/'runs.zip') as z:
        require(set(z.namelist())==set(manifest),'archive inventory')
        require(len(z.infolist())<=512 and sum(i.file_size for i in z.infolist())<=128*1024*1024,'archive cap')
        for name,m in manifest.items():
            require(not Path(name).is_absolute() and '..' not in Path(name).parts,'unsafe archive name')
            raw=z.read(name);require(len(raw)==m['bytes'] and digest(raw)==m['sha256'],'artifact checksum '+name)
        for case in report['cases']:
            name=case['case'];prefix=name+'/world/'
            transcript=[json.loads(line) for line in z.read(name+'/transcript.jsonl').splitlines()]
            require(len(transcript)==case['transcript_frames'] and len(transcript)<=512,'transcript count')
            events=[item['event'] for item in transcript if 'event' in item]
            barriers=[e for e in events if e['kind']=='probe_barrier']
            if name!='baseline':
                require(len(barriers)==1,'fault barrier absent')
                require(barriers[0]['payload']==case['fault']['barrier'],'barrier report differs')
            if name in ['worker_sources','worker_cells','sim_worker_cells']:
                require(barriers[0]['payload']['details']['cursor']>0,'not mid-page')
            if name in ['worker_sources','worker_cells']:
                started=[e['payload'] for e in events if e['kind']=='worker_started']
                require([e['ticket']['generation'] for e in started]==[1,2] and started[0]['worker_pid']!=started[1]['worker_pid'],'worker not replaced/fenced')
                require(sum(e['kind']=='attempt_failed' for e in events)==1,'failure not recorded')
            if name.startswith('sim_'):
                ready=[e for e in events if e['kind']=='ready']
                require(len(ready)==2 and ready[0]['sim_pid']!=ready[1]['sim_pid'],'new Sim absent')
            current=json.loads(z.read(prefix+'CURRENT.json'));raw=z.read(prefix+f"root-{current['slot']}.json")
            require(digest(raw)==current['root_sha256'],'CURRENT root checksum')
            root=json.loads(raw);require(root['worker_cache_bytes']==6*1024*1024 and root['background_reserve'] is None and root['wal_end']==0,'cache/root/checkpoint boundary')
            genesis=z.read(prefix+'genesis.json');require(digest(genesis)==root['genesis_sha256'],'genesis checksum')
            require(json.loads(genesis)['rules_identity']=='sim-w3:'+SOURCE,'rules provenance')
            ref=root['checkpoint'];raw=z.read(prefix+f"checkpoint-index-{ref['generation']}.json")
            require(digest(raw)==ref['sha256'],'checkpoint index checksum');index=json.loads(raw)
            raw=z.read(prefix+f"checkpoint-{ref['generation']}.json");require(digest(raw)==index['data_sha256'],'checkpoint data checksum');cp=json.loads(raw)
            require(cp['sequence']==root['sequence'] and cp['chain']==root['wal_chain'] and cp['genesis_sha256']==root['genesis_sha256'],'checkpoint root binding')
            require([r['operation_id'] for r in cp['receipts']]==index['operations'],'receipt index')
            receipt=[r for r in cp['receipts'] if r['operation_id']==0xe000000000000001]
            require(receipt==[case['receipt']['record']],'original admission record missing')
            engine=json.loads(bytes(cp['engine']));b=engine['background'];require(b['pending'] is None and b['version']==1,'durable task not consumed')
            sha=case['complete_air_state_sha256'];raw=z.read(prefix+'air-worker/candidates/'+sha+'.air');require(digest(raw)==sha,'worker candidate hash')
            candidate=json.loads(raw)['payload'];require(b['field']==candidate,'entire durable air state differs')
            field_hash=digest(b''.join(struct.pack('<I',n) for n in candidate['field']))
            require(field_hash==case['status']['background']['field']['field_sha256'],'reported field bits differ')
            world=engine['world'];world['revision']=0
            if reference_world is None: reference_world=world
            else: require(world==reference_world,'durable business differs after normalizing revision only')
            case_checks.append({'case':name,'frames':len(transcript),'whole_air_and_business_match':True,'original_receipt':True})
    if args.rerun:
        subprocess.run(['cargo','build','-p','mycity-headless','--release','--locked','--offline'],cwd=ROOT,check=True)
        with tempfile.TemporaryDirectory(prefix='mycity-w3-rerun-') as temporary:
            out=subprocess.run([str(ROOT/'target/release/mycity-headless'),'sim-probe',str(Path(temporary)/'run')],cwd=ROOT,capture_output=True,check=True)
            require(len(out.stdout)<=1024*1024,'rerun output limit');report_check(json.loads(out.stdout))
    result={'source_sha256':SOURCE,'release_tests_passed':sum(map(int,re.findall(r'test result: ok\. (\d+) passed;',log))),'cases':case_checks,'runs_zip_sha256':digest((HERE/'runs.zip').read_bytes()),'verifier_sha256':digest(Path(__file__).read_bytes()),'whole_process_group_measurement':'not_run','endurance_1800_seconds':'not_run','memory_16gb_gate':'not_run','W4':'not_accepted'}
    (HERE/'checks.json').write_text(json.dumps(result,indent=2)+'\n');print(json.dumps(result,indent=2))
if __name__=='__main__':main()
