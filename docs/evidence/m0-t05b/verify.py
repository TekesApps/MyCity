"""Recompute finite ledger evidence; never extrapolate to city balance or 16GB."""
from pathlib import Path
import hashlib
import json
import re

ROOT = Path(__file__).resolve().parents[3]
E = Path(__file__).resolve().parent
names = ['.cargo/config.toml', 'Cargo.toml', 'Cargo.lock', 'rust-toolchain.toml']
for directory in ['crates', 'fixtures']:
    names += [str(p.relative_to(ROOT)) for p in (ROOT / directory).rglob('*') if p.is_file()]
h = hashlib.sha256()
for name in sorted(names):
    b = (ROOT / name).read_bytes()
    n = name.encode()
    h.update(len(n).to_bytes(8, 'little')); h.update(n)
    h.update(len(b).to_bytes(8, 'little')); h.update(b)
v = json.loads((E / 'timeline.json').read_text())
p = json.loads((E / 'provenance.json').read_text())
assert h.hexdigest() == v['source_sha256'] == p['source_sha256']
assert hashlib.sha256((ROOT / 'Cargo.lock').read_bytes()).hexdigest() == v['lock_sha256'] == p['lock_sha256']
for name, digest in p['input_files'].items():
    assert hashlib.sha256((ROOT / name).read_bytes()).hexdigest() == digest

def cash(d):
    return {a['id']: a['cash'] for a in d['world']['accounts']}

for k, at in [('dense', 3600), ('economy', 86400), ('joint', 172800)]:
    d = v[k]
    assert sum(cash(d).values()) == 3000000
    assert d['progress']['last_complete'] == at
    assert d['progress']['phase'] is None and d['progress']['target'] is None
    assert d['continuation'] is None
    assert d['engine_owned_bytes'] <= 4 * 1024**2
    assert d['timeline_owned_bytes'] <= 2 * 1024**2
    assert d['checkpoint']['wal_bytes'] == 0
    assert d['checkpoint']['archive_bytes'] <= 128 * 1024**2
assert v['dense']['claim_count'] == 20000
assert v['dense']['claim_paid_cents'] == v['dense']['claim_principal_cents'] == 20000
assert cash(v['dense']) == {1: 1000000, 2: 200000 + 20000, 3: 600000 - 20000, 4: 1200000}
assert v['dense_pages'] == 79
# One economic day: wages8000, wage tax800, purchases4000, household6000/treasury4000 goods at2c/unit.
assert cash(v['economy']) == {1:1000000+800-8000, 2:200000+7200-12000, 3:600000-4000-8000+20000, 4:1200000+4000}
ec = v['economy']['world']['economy']
assert ec['closed_days'] == 1 and ec['consumed_f'] == 6000 and ec['consumed_g'] == 4000
# Joint independently rebuild each cash flow from initial balances.
expected = {1:1000000, 2:200000, 3:600000, 4:1200000, 5:0}
for payer, payee, cents in [(3,2,3000), (3,4,200), (2,4,2020), (3,5,112)]:
    expected[payer] -= cents; expected[payee] += cents
assert cash(v['joint']) == expected
w = v['joint']['world']; r = w['region']; x = w['exchange']; b = w['boot']
assert r['interaction_time'] == b['time'] == 172800
opening = json.loads((ROOT / 'fixtures/m0-exchange-opening.json').read_text())
initial_cities = {c['id']: c for c in opening['region']['cities']}
cs = {c['id']: c for c in r['cities'] if c}
assert [cs[i]['population'] for i in [1,2,3]] == [3990,110,300]
assert sum(c['population'] for c in cs.values()) == 4400
for i in [1,2,3]:
    assert cs[i]['development_elapsed'] == initial_cities[i]['development_elapsed'] + (172800 if i == 1 else 0)
assert not any(r['transits']) and not any(x['shipments'])
assert x['quotes'][0]['status'] == 'Delivered'
assert all(q is None or q['status'] == 'Expired' for q in x['quotes'][1:])
s = {s['id']:s for s in w['stocks']}
assert [s[i]['quantity'] for i in [1,2,3,4,5,6]] == [5000,0,600000,5400000,0,100000]
assert s[7]['quantity'] + s[8]['quantity'] == 5000
assert s[7]['basis'] == 1600 and s[8]['basis'] == 2020
assert b['fees'] == 200 and b['jobs'][0]['completed'] == 1
assert x['inbox']['accrued'] - x['inbox']['paid'] == 5
pay = v['joint']['payroll'][0]
assert pay['work_seconds'] == 16*3600
assert pay['work_seconds'] * 7 == pay['accrued'] * 3600 + pay['remainder']
assert pay['accrued'] == pay['billed'] == pay['paid'] == 112
assert pay['next_due'] is None
assert v['timeline_query_example']['complete_boundary'] is True
assert v['gpu'] == v['memory_16gb_gate'] == 'not_run'
assert p['host_memory_bytes'] == 32 * 1024**3
log = (E / 'tests.txt').read_text()
rows = re.findall(r'test result: ok\. (\d+) passed; (\d+) failed; (\d+) ignored;', log)
assert rows and all(int(f) == int(i) == 0 for _, f, i in rows)
passed = sum(int(n) for n, _, _ in rows)
assert passed == 185, passed
paced = json.loads(re.findall(r'T10_PACED_DAY (\{[^\n]+\})', log)[0])
assert paced['source_sha256'] == h.hexdigest() and paced['same_state_as_unpaced']
assert paced['target_exactly_met'] == (paced['actual_wall_seconds'] <= 60)
assert 'T05_ACTUAL_PROCESS_EXITS=33' in log
assert 'stale_advance_flood_cannot_spend_live_ticket_receipt_slots ... ok' in log
assert 'later_page_can_touch_all_sixty_four_admitted_accounts ... ok' in log
assert len(json.loads((E / 'gates.json').read_text())) == 3
assert all(g['exit_code'] == 0 for g in json.loads((E / 'gates.json').read_text()))
summary = {'source_sha256':h.hexdigest(), 'release_tests_passed':passed, 'actual_t05_process_exits':33,
           'finite_t05b_gate':'passed', 'full_product_t05':'not_completed', 'memory_16gb_gate':'not_run',
           'economic_balance':'not_verified', 'worker_pool':'not_implemented', 'gpu':'not_run',
           'legacy_profile_paced_day':paced, 'strict_economy_60_second_gate':'passed' if paced['target_exactly_met'] else 'not_met',
           'dense_actual_paid_cents':20000, 'dense_pages':v['dense_pages'], 'joint_cash_cents':expected,
           'dense_engine_owned_bytes':v['dense']['engine_owned_bytes'], 'probe_wall_seconds':v['wall_seconds'],
           'artifact_sha256':{name:hashlib.sha256((E/name).read_bytes()).hexdigest() for name in ['tests.txt','fmt.txt','clippy.txt','timeline.json','process.txt','provenance.json','gates.json']}}
(E / 'verified-summary.json').write_text(json.dumps(summary, indent=2) + '\n')
print(json.dumps(summary, indent=2))
