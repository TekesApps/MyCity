"""Verify frozen CPU-task evidence, checksums, source integration and numeric accounting."""
from pathlib import Path
import hashlib
import json
import math
import re
import struct
import zipfile

ROOT=Path(__file__).resolve().parents[3]
E=Path(__file__).resolve().parent
names=['.cargo/config.toml','Cargo.toml','Cargo.lock','rust-toolchain.toml']
for directory in ['crates','fixtures']:
    names += [str(p.relative_to(ROOT)) for p in (ROOT/directory).rglob('*') if p.is_file()]
h=hashlib.sha256()
for name in sorted(names):
    n=name.encode();b=(ROOT/name).read_bytes()
    h.update(len(n).to_bytes(8,'little'));h.update(n);h.update(len(b).to_bytes(8,'little'));h.update(b)
v=json.loads((E/'air.json').read_text());p=json.loads((E/'provenance.json').read_text())
ref=json.loads((E/'reference-checks.json').read_text())
assert h.hexdigest()==v['source_sha256']==p['source_sha256']
assert hashlib.sha256((ROOT/'Cargo.lock').read_bytes()).hexdigest()==v['lock_sha256']==p['lock_sha256']
assert hashlib.sha256((ROOT/'fixtures/air/run36-cpu.json').read_bytes()).hexdigest()==p['input_sha256']
assert hashlib.sha256((E/'tasks.zip').read_bytes()).hexdigest()==p['tasks_zip_sha256']==ref['tasks_zip_sha256']
assert hashlib.sha256((E/'reference.py').read_bytes()).hexdigest()==ref['reference_script_sha256']
assert len(v['cases'])==len(ref['cases'])==p['new_process_resumes']==5
assert v['memory_16gb_gate']==v['gpu']=='not_run'
assert v['worker_pool']==v['world_store_integration']=='not_implemented'
assert p['host_memory_bytes']==32*1024**3
MAGIC=b'MYCITY-AIR-TASK-1\n'
canonical=lambda x:json.dumps(x,ensure_ascii=False,separators=(',',':')).encode()
fields=[]
with zipfile.ZipFile(E/'tasks.zip') as archive:
    assert len(archive.infolist())==20
    for entry in archive.infolist():
        assert entry.file_size<8*1024**2+200
        assert hashlib.sha256(archive.read(entry)).hexdigest()==p['task_artifact_sha256'][entry.filename]
    for case,reference in zip(v['cases'],ref['cases']):
        name=case['name'];assert name==reference['name']
        i=json.loads(archive.read(name+'/input.json'))
        identity=hashlib.sha256(canonical(i)).hexdigest()
        report=case['report'];assert identity==report['input_sha256']==case['input_sha256']
        for stage in ['source','tile','complete']:
            b=archive.read(name+'/'+stage+'.air');offset=len(MAGIC)
            assert b[:offset]==MAGIC
            assert b[offset:offset+64].decode()==h.hexdigest()
            assert b[offset+64:offset+128].decode()==identity
            checkpoint=json.loads(b[offset+128:]);payload=checkpoint['payload']
            assert checkpoint['schema']==i['schema']=='run36-air-f32-v1'
            assert hashlib.sha256(canonical(payload)).hexdigest()==checkpoint['sha256']
            assert payload['input']==i and payload['input_sha256']==identity
            if stage=='source':
                assert payload['time']==0 and payload['progress']['phase']=='Sources'
                assert payload['progress']['cursor']==1 and payload['pending']['emitted_micro']>0
                assert any(payload['scratch'])
            elif stage=='tile':
                assert payload['time']==0 and payload['progress']['phase']=='Cells'
                assert payload['progress']['cursor']==17
                assert not any(payload['integral'])
            else:
                assert payload['time']==i['target'] and payload['progress'] is None
                field=payload['field'];integral=payload['integral']
                assert len(field)==len(integral)==i['side']**2
                assert hashlib.sha256(b''.join(struct.pack('<I',b) for b in field)).hexdigest()==report['field_sha256']
                assert hashlib.sha256(b''.join(struct.pack('<Q',b) for b in integral)).hexdigest()==report['integral_sha256']
                floats=[struct.unpack('<f',struct.pack('<I',b))[0] for b in field]
                assert all(math.isfinite(x) and x>=0 for x in floats)
                assert abs(sum(floats)-report['sum'])<1e-6
        emitted=sum(s['rate_micro']*(s['end']-s['start']) for s in i['sources'])
        assert report['emitted_micro']==emitted
        assert report['time']==report['target']==i['target'] and report['complete']
        assert report['steps']==reference['steps']
        assert report['owned_bytes']<=2*1024**2 and case['source_struct_bytes']==20
        assert case['same_state_after_source_and_tile_file_restore']
        assert case['new_process_resume']['source_sha256']==h.hexdigest()
        child=case['new_process_resume']['report']
        for key in ['field_sha256','integral_sha256','emitted_micro']:
            assert child[key]==report[key]
        # Reports are display floats; authoritative checkpoint numbers are integer bits.
        for key in ['decay','background','boundary_exchange','rounding_residual']:
            assert abs(child[key]-report[key])<=max(1e-12,abs(report[key])*1e-15)
        initial=i['side']**2*i['initial_micro']/1e6
        closure=report['sum']-(initial+emitted/1e6-report['decay']+report['background']+report['boundary_exchange']+report['rounding_residual'])
        assert abs(closure)<1e-4,(name,closure)
        assert reference['passed'] and reference['max_field_normalized_error']<=1 and reference['max_time_mean_normalized_error']<=1
        assert case['comparison']['passed'] and case['comparison']['max_normalized_error']<=1
        fields.append({'name':name,'side':i['side'],'game_seconds':i['target'],'steps':report['steps'],'sources':len(i['sources']),'emitted_micro':emitted,'numeric_ledger_residual':closure,'owned_bytes':report['owned_bytes'],'field_error':reference['max_absolute_field_error'],'time_mean_error':reference['max_absolute_time_mean_error']})
log=(E/'tests.txt').read_text()
rows=re.findall(r'test result: ok\. (\d+) passed; (\d+) failed; (\d+) ignored;',log)
assert rows and all(int(f)==int(i)==0 for _,f,i in rows)
passed=sum(int(n) for n,_,_ in rows);assert passed==195,passed
assert 'decoder_rejects_excess_rows_before_accepting_source_or_checkpoint_vectors ... ok' in log
assert 'T05_ACTUAL_PROCESS_EXITS=33' in log
paced=json.loads(re.findall(r'T10_PACED_DAY (\{[^\n]+\})',log)[0])
assert paced['source_sha256']==h.hexdigest() and paced['same_state_as_unpaced']
gates=json.loads((E/'gates.json').read_text());assert len(gates)==3 and all(g['exit_code']==0 for g in gates)
summary={'source_sha256':h.hexdigest(),'release_tests_passed':passed,'finite_air_cpu_gate':'passed','full_t12':'not_completed','memory_16gb_gate':'not_run','world_integration':'not_implemented','gpu':'not_run','worker_pool':'not_implemented','new_process_resumes':5,'cases':fields,'probe_wall_seconds':v['wall_seconds'],'rss_scope':p['rss_scope'],'strict_legacy_economy_60_second_gate':'passed' if paced['target_exactly_met'] else 'not_met','legacy_paced_day':paced,'artifact_sha256':{name:hashlib.sha256((E/name).read_bytes()).hexdigest() for name in ['tests.txt','fmt.txt','clippy.txt','air.json','tasks.zip','reference-checks.json','provenance.json','process.txt','gates.json']}}
(E/'verified-summary.json').write_text(json.dumps(summary,indent=2)+'\n')
print(json.dumps(summary,indent=2))
