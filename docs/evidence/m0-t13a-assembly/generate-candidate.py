#!/usr/bin/env python3
"""Versioned candidate assembly and independent arithmetic, not a passing load."""
import copy
import hashlib
import json
from pathlib import Path

ROOT = Path(__file__).resolve().parents[3]
p = json.loads((ROOT / 'fixtures/m0-background-joint-opening.json').read_bytes())
acceptance = json.loads((ROOT / 'fixtures/m0-t13a-acceptance.json').read_bytes())
p['boot']['profile'] = 'pboot-cold-scale-v1'
p['exchange']['profile'] = 'regional-fulfilment-scale-v1'
p['accounts'] = p['accounts'][:4] + [
    {'id': i, 'role': 'household', 'cash': 0, 'source': i} for i in range(5, 65)]
for a in p['accounts']:
    a['source'] = a['id']
p['stocks'] += [{'id': i, 'owner': 3, 'commodity': 'raw', 'quantity': 0, 'basis': 0, 'source': 1000+i}
                for i in range(15, 129)]
p['stocks'][14].update(quantity=1000, basis=300)
for s in p['stocks']:
    s['source'] = 1000 + s['id']
p['stocks'][6].update(quantity=20000, basis=8000)
p['sources'] = [{'id': a['source'], 'kind': 'cash_capital', 'amount': a['cash']} for a in p['accounts']]
p['sources'] += [{'id': s['source'], 'kind': 'inventory_capital', 'amount': s['basis']} for s in p['stocks']]
p['region']['cities'] += [{'id': i, 'development_elapsed': 0, 'population': 100, 'willing': 0,
                          'housing_capacity': 1000} for i in range(4, 9)]
p['region']['obligations'] = [{'id': i, 'payer': 3, 'payee': 1, 'period_seconds': 86400, 'amount': 100,
                            'next_due': 86400, 'expires_at': 30*86400, 'arrears': 0} for i in range(1, 17)]
p['exchange']['maps'] += [{'city': i, 'origin_x_cm': (i-1)*102400} for i in range(4, 9)]
p['exchange']['stocks'][6]['limit'] = 20000
q = copy.deepcopy(p['exchange']['quotes'][0])
p['exchange']['quotes'] = []
for i in range(1, 9):
    item = copy.deepcopy(q)
    item.update(id=i, destination=8+(i-1)%4, expires_at=16*86400)
    p['exchange']['quotes'].append(item)
pol = {'schema': 'mycity-m0-t13a-real-policy-v1-candidate', 'status': 'frozen_assembly_input_not_performance_accepted',
       'game_seconds': 30*86400, 'initial_cash_cents': sum(a['cash'] for a in p['accounts']),
       'initial_account_count': 64, 'initial_stock_count': 128,
       'payroll': [{'id': i, 'payer': 3, 'payee': 5+(i-1)%60, 'home_city': 1, 'hourly_cents': 7+i%3,
                    'cycle': [{'kind': 'daily'}, {'kind': 'weekly'}, {'kind': 'monthly', 'anchor': 1}][(i-1)%3],
                    'ends_at': 30*86400} for i in range(1, 65)],
       'claims': [{'first_id': i*1000+1, 'count': 1000, 'payer': 3, 'payee': 5+i, 'amount': 1,
                   'due': (i+1)*86400} for i in range(20)],
       'boot_start_seconds': {'pump': 0, 'generator': 60},
       'first_group_settle_seconds': 180,
       'freight_dispatch': [{'at': 60 if day == 0 else day*86400, 'quotes': list(range(start, start+4))}
                            for day, start in [(0, 1), (15, 5)]],
       'inactive_city': {'city': 1, 'switch_to': 2, 'closed_at': 7*86400, 'opened_at': 8*86400}}
work_hours = 29*8
paid = [x['hourly_cents']*work_hours for x in pol['payroll']]
cash = {a['id']: a['cash'] for a in p['accounts']}
for spec, amount in zip(pol['payroll'], paid):
    cash[spec['payer']] -= amount
    cash[spec['payee']] += amount
for spec in pol['claims']:
    amount = spec['count']*spec['amount']
    cash[spec['payer']] -= amount
    cash[spec['payee']] += amount
cash[3] -= 48000+500
cash[1] += 48000
cash[2] -= 8*(2020+5)
cash[4] += 8*(2020+5)+500
assert sum(cash.values()) == pol['initial_cash_cents'] == 3000000 and min(cash.values()) >= 0
oracle = {'scope': 'independent_expected_arithmetic_only_not_runtime',
          'expected_work_seconds_each': work_hours*3600, 'expected_payroll_paid': paid,
          'expected_wages_paid_total': sum(paid), 'expected_claims_paid_total': 20000,
          'expected_tax_paid': 48000, 'expected_boot_fees': 500, 'expected_freight_invoices': 16000,
          'expected_freight_fees': 160, 'expected_inbox_fees': 40, 'expected_raw_source_quantity': 12000,
          'expected_raw_source_basis': 4800, 'expected_extra_stock_15': {'quantity': 1000, 'basis': 300}, 'expected_cash_accounts': [{'id': i, 'cash': v} for i, v in sorted(cash.items())],
          'expected_cash_total': sum(cash.values()), 'acceptance_sha256': hashlib.sha256(
              (ROOT/'fixtures/m0-t13a-acceptance.json').read_bytes()).hexdigest()}
for name, obj in [('m0-t13a-full-opening-candidate.json', p), ('m0-t13a-real-policy-candidate.json', pol)]:
    (ROOT/'fixtures'/name).write_text(json.dumps(obj, separators=(',', ':'))+'\n')
(ROOT/'fixtures/m0-t13a-expected-oracle-candidate.json').write_text(json.dumps(oracle, indent=2)+'\n')
print(json.dumps({'counts': {'accounts': len(p['accounts']), 'stocks': len(p['stocks']), 'cities': len(p['region']['cities']),
                             'maps': len(p['exchange']['maps']), 'obligations': len(p['region']['obligations']),
                             'payroll': len(pol['payroll']), 'claims': sum(x['count'] for x in pol['claims'])},
                  'input_bytes': (ROOT/'fixtures/m0-t13a-full-opening-candidate.json').stat().st_size,
                  'oracle_totals': {k:v for k,v in oracle.items() if k.endswith('total')},
                  'runtime_acceptance': 'not_run'}, indent=2))
