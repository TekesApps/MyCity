"""Independent fixed-input recurrence, cents/stock oracle and v7 checkpoint digest.
No Store open or model command. Existing published canonicalization is hash-pinned.
"""
from pathlib import Path
import hashlib, importlib.util, json, sys

report_path, world, reader_path = map(Path, sys.argv[1:])
r = json.loads(report_path.read_text())
old = Path(__file__).parents[1] / 'core-wage-spatial-156' / 'check_checkpoint.py'
assert hashlib.sha256(old.read_bytes()).hexdigest() == '8740ca0a181b0ae82d4a2f2d0d52c7f1a9b64f4b9f3468c9ac63dedab3da55f9'
spec = importlib.util.spec_from_file_location('canonical156', old)
m = importlib.util.module_from_spec(spec); spec.loader.exec_module(m)
base = r['scenario'] == 'base'
a = {1:1000000, 2:200000 if base else 0, 3:600000, 4:1200000}
initial = sum(a.values()); raw, product = 280, 0
retained = 0; tax_total = 0; tax_due = 0; produced = 0; sales = 0
rows = []
def transfer(payer, payee, cents):
    assert cents >= 0 and a[payer] >= cents
    a[payer] -= cents; a[payee] += cents
transfer(1,3,10000); transfer(4,3,1000)
for day in range(1,30):
    transfer(3,1,tax_due)
    wage = 800 if day <= 28 else 0
    transfer(3,2,wage)
    if day == 14: transfer(3,4,1000)
    made = 10 if product == 0 and raw >= 10 and day <= 28 else 0
    raw -= made; product += made; produced += made
    transfer(3,1,made) # fixed fixture production utility1c/q
    sold = min(10,product,a[2]//102)
    transfer(2,3,100*sold); transfer(2,1,2*sold)
    product -= sold; sales += sold
    capitalized = 800 if made else 0
    idle = wage-capitalized
    margin = 9*sold-idle # sold basis91c, shelf100c; no expensing unsold basis twice
    tax_due = max(0,margin)//2
    tax_total += tax_due; retained += margin-tax_due
    rows.append({'day':day,'produced':made,'sales':sold,'idle_accrued_labor':idle,
                 'margin':margin,'tax':tax_due,'retained':retained})
assert tax_due == 0
if base:
    assert (produced,sales,raw,product,tax_total,retained)==(280,280,0,0,1260,1260)
    # Independent largest-remainder allocation of retained1260 to6028/100 shares.
    shares={2:6028,1:100}; payouts={k:retained*v//6128 for k,v in shares.items()}
    tail=retained-sum(payouts.values())
    order=sorted(shares,key=lambda k:-(retained*shares[k]%6128))
    for k in order[:tail]: payouts[k]+=1
    assert payouts == {2:1239,1:21}
    for party,amount in payouts.items(): transfer(3,party,amount)
    transfer(1,4,21); transfer(3,2,602800); transfer(3,1,10000)
    expected_phase='Closed'; expected_capital=612800; expected_dividend=1260
    expected={1:1002100,2:797879,3:0,4:1200021}
else:
    assert (produced,sales,raw,product,tax_total,retained)==(220,219,60,1,938,-3767)
    expected_phase='Liquidating'; expected_capital=0; expected_dividend=0
    expected={1:991596,2:62,3:608342,4:1200000}
assert a == expected and sum(a.values()) == initial
assert r['sales_by_day'] == [row['sales'] for row in rows]
for key,value in {'sales_total':sales,'production_total':produced,'raw_remaining':raw,
  'product_remaining':product,'tax_assessed_cents':tax_total,'tax_paid_cents':tax_total,
  'retained_earnings_after_day29_cents':retained,'dividend_total_cents':expected_dividend,
  'capital_returned_cents':expected_capital,'exit_phase':expected_phase,
  'seller_payroll_accrued_cents':22400,'seller_payroll_paid_cents':22400,
  'total_cash_final_cents':initial,'loan_outstanding_after_day29_cents':0}.items():
    assert r[key]==value,(key,r[key],value)
assert {x['id']:x['cash'] for x in r['final_accounts']} == expected
assert r['market_utility_payments_total_cents']==produced+sales
assert r['production_utility_cost_cents']==produced and r['buyer_utility_surcharge_cents']==sales
cp_path,cp=m.latest_checkpoint(world); e=m.engine_bytes(cp); w=e['world']; t=w['town']['state']
accounts={slot['value']['id']:slot['value']['cash'] for slot in w['accounts']['slots'] if slot['value']}
assert accounts == expected
stocks={slot['value']['id']:slot['value'] for slot in w['stocks']['slots'] if slot['value']}
assert (stocks[2]['quantity'],stocks[2]['basis'])==(raw,raw*10)
assert (stocks[3]['quantity'],stocks[3]['basis'])==(product,product*91)
assert all(c['paid']==c['principal'] and c['processed'] for c in e['timeline']['claims'])
tax_sources=[s['source']['details'] for s in e['timeline']['claim_sources'] if s['source']['kind']=='tax']
positive=[row for row in rows if row['tax']>0]
assert [s['assessed'] for s in tax_sources]==[row['tax'] for row in positive]
assert [s['taxable_basis'] for s in tax_sources]==[row['margin'] for row in positive]
assert len(tax_sources)==(28 if base else 22)
assert sum(c['principal'] for c in e['timeline']['claims'])==tax_total+1000
reg=t['equity_registers'][0]; holders={h['account']:h['shares'] for h in reg['holders']}
assert reg['issued_shares']==6128 and holders=={2:6028,1:100}
assert reg.get('cumulative_dividends_distributed', 0)==expected_dividend
assert reg.get('cumulative_capital_returned', 0)==expected_capital
assert reg.get('cumulative_liquidation_surplus_distributed', 0)==0
assert t['tax_cursors'][0]['retained_earnings_cents']==(0 if base else retained)
assert t['exit']['phase']==expected_phase.lower()
genesis=json.loads((world/'genesis.json').read_bytes())
assert genesis['schema']=='mycity-m0-genesis-v1'
engine_hash,state_hash=m.view_hashes(e,{'spatial':genesis.get('spatial')})
assert state_hash==r['final_state_hash'] and cp['sequence']==r['final_sequence']
reader=json.loads(next(x.split('=',1)[1] for x in reader_path.read_text().splitlines()
                      if x.startswith('TOWN_EQUITY_28D_VERIFY=')))
assert reader['business_commands_executed']==0
assert reader['state_hash']==state_hash and reader['committed_sequence']==cp['sequence']
assert {x['id']:x['cash_cents'] for x in reader['accounts']}==expected
print(json.dumps({'passed':True,'scenario':r['scenario'],'cash':expected,'cash_total':initial,
  'produced':produced,'sales':sales,'raw':raw,'product':product,'tax':tax_total,
  'retained_before_distribution':retained,'capital_returned':expected_capital,
  'dividend':expected_dividend,'exit_phase':expected_phase,'checkpoint':str(cp_path),
  'checkpoint_sha256':hashlib.sha256(cp_path.read_bytes()).hexdigest(),
  'sequence':cp['sequence'],'state_hash':state_hash,'engine_hash':engine_hash,
  'scope':'independent recurrence/literal cents, stock/claims/register and checkpoint digest; no WAL replay'},indent=2))
