#!/usr/bin/env python3
"""Frozen W2 source, durable world, actual worker artifacts and independent f64 check."""
import argparse
import hashlib
import json
from pathlib import Path
import re
import struct
import subprocess
import tempfile
import zipfile
HERE = Path(__file__).resolve().parent
ROOT = HERE.parents[2]
def require(ok, message):
    if not ok:
        raise SystemExit(message)
def digest(data):
    return hashlib.sha256(data).hexdigest()
def source_hash():
    names = ['.cargo/config.toml','Cargo.toml','Cargo.lock','rust-toolchain.toml']
    names += [p.relative_to(ROOT).as_posix() for folder in ['crates','fixtures'] for p in (ROOT/folder).rglob('*') if p.is_file()]
    h=hashlib.sha256()
    for name in sorted(names):
        b=name.encode();data=(ROOT/name).read_bytes()
        h.update(struct.pack('<Q',len(b)));h.update(b);h.update(struct.pack('<Q',len(data)));h.update(data)
    return h.hexdigest()
def bits_hash(values, width):
    return digest(b''.join(struct.pack('<'+width,n) for n in values))
def report_check(r):
    require(r['source_sha256']==source_hash(),'frozen source differs')
    require(len(r['jobs'])==4 and r['field']['time']==120,'four windows not complete')
    require(r['field']['emitted_micro']==120000,'actual freight emission proxy mismatch')
    require(r['quote1']=={'delivered':1000,'invoice':2000,'freight':20,'status':'Delivered'},'actual fulfilment mismatch')
    require(sum(a['cash'] for a in r['world']['accounts'])==r['initial_total_cash_cents'],'cash not conserved')
    require(r['checkpoint_reopen_original_receipt'],'no original receipt proof')
    for name in ['independent_sim','joint_1800_seconds','whole_process_group_resources','memory_16gb_gate']:
        require(r[name] in ['not_implemented','not_run'],'unverified scope claimed')
    require(r['capacity_after_checkpoint']['background_reserved_bytes']==0 and r['capacity_after_checkpoint']['continuation_reserved_bytes']==0,'reservation not released')
def bounded_archive(path):
    z=zipfile.ZipFile(path)
    require(len(z.infolist())<=64,'too many artifacts')
    require(all(i.file_size<=16*1024*1024 for i in z.infolist()),'oversized artifact')
    return z
def independent_reference(report, inputs, result):
    n=128;pitch=n+2
    cells=[y*pitch+x for y in range(1,n+1) for x in range(1,n+1)]
    field=[2.0]*(pitch*pitch);integrals=[0.0]*len(cells);at=0;emitted=0
    for job,state in zip(report['jobs'],inputs):
        i=state['input'];t=job['ticket']
        require(t['start']==at and t['end']==i['target'] and t['field_version']==report['jobs'].index(job),'window/version discontinuity')
        require(i['side']==128 and i['boundary_micro']==[2000000]*4,'unexpected reference profile')
        anchors=sorted({at,i['target'],*[s['start'] for s in i['sources']],*[s['end'] for s in i['sources']],*range((at//60+1)*60,i['target'],60)})
        for end in anchors[1:]:
            dt=end-at;h=dt/60;r=.02*h;d=.01*h
            # Independent f64, unpaged, algebraically reordered reference.
            keep=(1-d)*(1-4*r);near=(1-d)*r;regen=2*d;next_field=field[:]
            for p in cells:
                next_field[p]=keep*field[p]+near*(field[p-pitch]+field[p+pitch]+field[p-1]+field[p+1])+regen
            for s in i['sources']:
                quantity=s['rate_micro']*max(0,min(end,s['end'])-max(at,s['start']))
                cell=s['cell'];next_field[(cell//n+1)*pitch+cell%n+1]+=quantity/1e6;emitted+=quantity
            for c,p in enumerate(cells):integrals[c]+=(field[p]+next_field[p])*dt/2
            field=next_field;at=end
    actual=[struct.unpack('<f',struct.pack('<I',b))[0] for b in result['field']]
    actual_i=[struct.unpack('<d',struct.pack('<Q',b))[0] for b in result['integral']]
    maximum=max(abs(v-field[p]) for v,p in zip(actual,cells))
    mean_maximum=max(abs(v-ref)/at for v,ref in zip(actual_i,integrals))
    require(all(abs(v-field[p])<=.001+.0001*abs(field[p]) for v,p in zip(actual,cells)),'independent field error')
    require(all(abs(v-ref)/at<=.001+.0001*abs(ref/at) for v,ref in zip(actual_i,integrals)),'independent integral error')
    require(emitted==report['field']['emitted_micro'],'independent source accumulation')
    return {'scope':'independent continuous f64 numerical reference only','max_absolute_field_error':maximum,'max_absolute_time_mean_error':mean_maximum,'emitted_micro':emitted,'physical_accuracy':'not_verified'}
def main():
    parser=argparse.ArgumentParser();parser.add_argument('--rerun',action='store_true');args=parser.parse_args()
    report=json.loads((HERE/'probe.json').read_bytes());report_check(report)
    gates=json.loads((HERE/'gates.json').read_bytes())
    require({g['gate'] for g in gates}=={'fmt','clippy','release-tests'} and all(g['exit_code']==0 for g in gates),'incomplete release gate')
    log=(HERE/'release-tests.txt').read_text()
    require('BACKGROUND_ACTUAL_PROCESS_EXITS import=7 accept=11' in log,'actual process fault matrix absent')
    require('BACKGROUND_ASSET_ROTATION accepted=33 max_live_assets=32' in log,'asset rotation absent')
    require('actual_source_cleanup=25' in log,'protected control concurrency absent')
    require('test result: FAILED' not in log,'failed release tests')
    inputs=[]
    with bounded_archive(HERE/'tasks.zip') as z:
        for job in report['jobs']:
            ticket=job['ticket'];raw=z.read(f"tasks/inputs/{ticket['checkpoint_sha256']}.air")
            require(digest(raw)==ticket['checkpoint_sha256'],'input asset checksum')
            cp=json.loads(raw);i=cp['payload']['input']
            require(digest(json.dumps(i,separators=(',',':')).encode())==ticket['input_sha256'],'input identity')
            inputs.append(cp['payload'])
            result_raw=z.read(f"tasks/candidates/{job['candidate_sha256']}.air")
            require(digest(result_raw)==job['candidate_sha256'],'candidate checksum')
            state=json.loads(result_raw)['payload']
            require(state['time']==ticket['end'] and state['progress'] is None,'partial worker result')
            require(bits_hash(state['field'],'I')==job['result']['field_sha256'],'field bits report mismatch')
            require(bits_hash(state['integral'],'Q')==job['result']['integral_sha256'],'integral report mismatch')
        final_worker=state
    with bounded_archive(HERE/'world.zip') as z:
        current=json.loads(z.read('world/CURRENT.json'));raw=z.read(f"world/root-{current['slot']}.json")
        require(digest(raw)==current['root_sha256'],'CURRENT checksum')
        root=json.loads(raw)
        genesis_raw=z.read('world/genesis.json');require(digest(genesis_raw)==root['genesis_sha256'],'genesis checksum')
        genesis=json.loads(genesis_raw);require(genesis['rules_identity']==report['rules_identity'],'rules identity')
        opening=json.loads(bytes(genesis['opening']));quote=opening['exchange']['quotes'][0]
        require(all(s['id']==quote['id'] and s['rate_micro']==quote['quantity'] and s['cell']==16*128+16 for state in inputs for s in state['input']['sources']),'source not bound to actual quote/warehouse')
        ref=root['checkpoint'];index_raw=z.read(f"world/checkpoint-index-{ref['generation']}.json")
        require(digest(index_raw)==ref['sha256'],'checkpoint index')
        index=json.loads(index_raw);cp_raw=z.read(f"world/checkpoint-{ref['generation']}.json")
        require(digest(cp_raw)==index['data_sha256'],'checkpoint data')
        cp=json.loads(cp_raw);require(cp['genesis_sha256']==root['genesis_sha256'] and cp['sequence']==root['sequence'] and cp['chain']==root['wal_chain'],'checkpoint boundary')
        require([r['operation_id'] for r in cp['receipts']]==index['operations'],'receipt index differs')
        engine=json.loads(bytes(cp['engine']))
        b=engine['background'];require(b['pending'] is None and b['version']==4,'task not consumed')
        # Exported Engine Value keys are ordered differently; JSON object equality preserves bits.
        require(b['field']==final_worker,'durable field differs from actual candidate')
        accounts=[s['value'] for s in engine['world']['accounts']['slots'] if s['value'] is not None]
        require(accounts==report['world']['accounts'],'durable accounts differ from report')
        require(root['wal_end']==0 and root['background_reserve'] is None,'checkpoint not complete')
        require(not any(Path(i.filename).name.startswith('air-result-') for i in z.infolist()),'retired assets not reclaimed')
    reference=independent_reference(report,inputs,final_worker)
    if args.rerun:
        subprocess.run(['cargo','build','-p','mycity-headless','--release','--locked','--offline'],cwd=ROOT,check=True)
        with tempfile.TemporaryDirectory(prefix='mycity-w2-rerun-') as temporary:
            out=subprocess.check_output([str(ROOT/'target/release/mycity-headless'),'background-probe',str(Path(temporary)/'run')],cwd=ROOT)
            report_check(json.loads(out))
    result={'source_sha256':report['source_sha256'],'release_tests_passed':sum(map(int,re.findall(r'test result: ok\. (\d+) passed;',log))),
        'reference':reference,'verifier_sha256':digest(Path(__file__).read_bytes()),'tasks_zip_sha256':digest((HERE/'tasks.zip').read_bytes()),'world_zip_sha256':digest((HERE/'world.zip').read_bytes()),'memory_16gb_gate':'not_run','W3_W4':'not_accepted'}
    (HERE/'checks.json').write_text(json.dumps(result,indent=2)+'\n');print(json.dumps(result,indent=2))
if __name__=='__main__':main()
