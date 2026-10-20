#!/usr/bin/env python3
"""Independent full-record arithmetic/provenance audit, only for T13A-1a CPU reference.
No throughput, steady, full L1, 8h or 16GB gate is signed here.
"""
import copy
import hashlib
import json
import re
import struct
import zipfile
from pathlib import Path
HERE=Path(__file__).resolve().parent
ROOT=HERE.parents[2]
SOURCE='62a57f1cd6fdc9911e1e408cb5650bea2f0b8cbefefd16276adf0c8299551bdf'
def sha(b):return hashlib.sha256(b).hexdigest()
def require(ok,why):
    if not ok:raise SystemExit(why)
def source_hash():
    names=['.cargo/config.toml','Cargo.toml','Cargo.lock','rust-toolchain.toml']
    names += [p.relative_to(ROOT).as_posix() for d in ['crates','fixtures'] for p in (ROOT/d).rglob('*') if p.is_file()]
    h=hashlib.sha256()
    for name in sorted(names):
        n=name.encode();b=(ROOT/name).read_bytes()
        h.update(struct.pack('<Q',len(n))+n+struct.pack('<Q',len(b))+b)
    return h.hexdigest()
def slots(a):return [s['value'] for s in a['slots'] if s['value'] is not None]
def main():
    require(source_hash()==SOURCE,'source changed; use frozen release checkout')
    manifest=json.loads((HERE/'reference-manifest.json').read_bytes())
    with zipfile.ZipFile(HERE/'reference.zip') as z:
        require(sorted(z.namelist())==sorted(manifest),'unexpected artifact member')
        raw={name:z.read(name) for name in manifest}
    for name,b in raw.items():require(sha(b)==manifest[name]['sha256'] and len(b)==manifest[name]['bytes'],'artifact mismatch '+name)
    r=json.loads(raw['report.json']);e=json.loads(raw['final-engine.json'])
    require(r['source_sha256']==SOURCE and sha(raw['final-engine.json'])==r['final_checkpoint_sha256'],'source/checkpoint mismatch')
    for label,name in [('opening','full-opening'),('policy','real-policy'),('oracle','expected-oracle')]:
        require(sha((ROOT/f'fixtures/m0-t13a-{name}-candidate.json').read_bytes())==r[label+'_sha256'],label+' mismatch')
    opening=json.loads((ROOT/'fixtures/m0-t13a-full-opening-candidate.json').read_bytes())
    policy=json.loads((ROOT/'fixtures/m0-t13a-real-policy-candidate.json').read_bytes())
    w=e['world'];accounts=slots(w['accounts']);stocks=slots(w['stocks']);t=e['timeline'];x=w['exchange']
    require((len(accounts),len(stocks),len(t['payroll']),len(t['claims']),len(x['maps']))==(64,128,64,20000,8),'full counts')
    require(len([c for c in w['region']['cities'] if c])==8 and t['progress']['last_complete']==2592000,'city/calendar coverage')
    cash={a['id']:a['cash'] for a in opening['accounts']}
    for p in t['payroll']:
        spec=policy['payroll'][p['spec']['id']-1];paid=spec['hourly_cents']*29*8
        require(p['paid']==p['accrued']==paid and p['work_seconds']==29*8*3600,'wages/work differs')
        cash[spec['payer']]-=paid;cash[spec['payee']]+=paid
    require(sum(p['paid'] for p in t['payroll'])==118784,'total wage')
    require([c['id'] for c in t['claims']]==list(range(1,20001)),'claim identities')
    for c in t['claims']:
        require(c['processed'] and c['principal']==c['paid']==1,'claim not paid exactly')
        require(c['payee']==5+(c['id']-1)//1000 and c['payer']==3 and c['due']==((c['id']-1)//1000+1)*86400,'claim origin differs')
        cash[c['payer']]-=1;cash[c['payee']]+=1
    cash[3]-=48000+500;cash[1]+=48000;cash[2]-=8*(2000+20+5);cash[4]+=8*(2000+20+5)+500
    require(cash=={a['id']:a['cash'] for a in accounts} and sum(cash.values())==3000000,'independent cash differs')
    require(all(a['protected']==0 for a in accounts),'protected funds leaked')
    require((stocks[6]['quantity'],stocks[6]['basis'],stocks[6]['offset'])==(12000,4800,8000),'raw source pool differs')
    for i in range(7,11):
        s=stocks[i];require((s['quantity'],s['basis'],s['origin_quantity'],s['origin_basis'],s['offset'],s['lot_id'])==(2000,4040,2000,4040,0,0xc0000000+i-2),'destination pool differs')
    for s,initial in zip(stocks[14:],opening['stocks'][14:]):
        for key in ['id','commodity','quantity','basis']:require(s[key]==initial[key],'extra inventory changed')
        require(s['origin_quantity']==initial['quantity'] and s['origin_basis']==initial['basis'] and s['offset']==0,'extra provenance changed')
        require(s['owner']['raw']=={'world':1,'branch':1,'domain':1,'index':2,'generation':1},'extra owner handle changed')
    require(not any(x['shipments']) and not any(e['leases']) and len(e['resources'])==8,'live resource leaked')
    require(all(r['reserved']==r['occupied']==0 for r in e['resources']),'pool reservation leaked')
    for i,q in enumerate(x['quotes']):
        require(q['status']=='Delivered' and q['delivered']==1000 and q['invoice']==2000 and q['freight']==20 and q['source_cost']==400,'actual delivery differs')
        f=q['source_fragment'];require((f['origin_quantity'],f['origin_basis'],f['offset'],f['quantity'],f['basis'])==(20000,8000,i*1000,1000,400),'source interval differs')
    obligations=[o for o in w['region']['obligations'] if o]
    require(len(obligations)==16 and all(o['arrears']==0 and o['next_due'] is None and o['expires_at']==2592000 for o in obligations),'periodic obligation not settled/expired')
    require(x['inbox']['closed']==8 and x['inbox']['accrued']==x['inbox']['paid']==40 and not any(x['inbox']['facts']+x['inbox']['retained']),'inbox recovery differs')
    business={key:copy.deepcopy(e[key]) for key in ['world','resources','leases','next_token','timeline']}
    business['world']['revision']=0;business['schema']='mycity-business-comparison-v1'
    require(sha(json.dumps(business,sort_keys=True,separators=(',',':')).encode())==r['business_sha256'],'full business digest')
    require(r['field']['emitted_micro']==960000 and r['field']['side']==128 and r['field']['time']==2592000,'air reference incomplete')
    require(r['memory_16gb_gate']==r['eight_hour_gate']==r['performance_acceptance']=='not_run' and r['full_M0']=='not_accepted','overclaim')
    log=(HERE/'tests-release.txt').read_text();require('FAILED' not in log and 'error:' not in log,'test failure remains')
    count=sum(map(int,re.findall(r'test result: ok\. (\d+) passed;',log)))
    require(count>=240,'incomplete test suite')
    checks={'schema':'mycity-t13a-assembly-check-v1','source_sha256':SOURCE,'scope':'T13A-1a_full_direct_CPU_and_native_layout_only','full_30_game_day_arithmetic_and_checkpoint':'passed','full_workspace_release_tests':count,'real_process_exit_points_extra_inventory':7,'full_L1_pressure':'not_run','long_observer':'not_implemented','steady_gate':'not_run','eight_hour_gate':'not_run','memory_16gb_gate':'not_run','full_M0':'not_accepted'}
    (HERE/'checks.json').write_text(json.dumps(checks,indent=2)+'\n');print(json.dumps(checks,indent=2))
if __name__=='__main__':main()
