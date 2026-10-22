from pathlib import Path
import json,hashlib
r=Path(__file__).resolve().parent
out=[]
for label,day in [('96-town-employment-final-default',10),('97-town-employment-day01',1),('98-town-employment-day27',27)]:
 receipt=json.loads((r/label/'exit.json').read_text()); assert receipt['exit_code']==0 and receipt['source_unchanged']
 a=json.loads((r/label/'stdout.log').read_text()); assert a['days']==28 and a['switch_day']==day
 assert len(a['scenarios'])==6 and a['continuous_self_funding_demonstrated'] is False
 rows=a['scenarios']
 for x in rows:
  total=x['totals']; old_days=day if x['changed'] else 28; new_days=28-day if x['changed'] else 0
  assert x['old_work_seconds_close']==old_days*28800 and x['new_work_seconds']==new_days*28800
  assert x['old_accrued']==old_days*8*x['old_wage_cents_per_hour']
  assert x['new_accrued']==new_days*8*x['new_wage_cents_per_hour']
  assert x['old_arrears_close']==x['old_accrued']-x['old_paid'] and x['new_arrears']==x['new_accrued']-x['new_paid']
  assert x['household_close']==x['household_open']+x['old_paid']+x['new_paid']-total['goods_payments']-total['sold']*2
  assert x['producer_close']==x['producer_open']+total['goods_payments']-x['retail_goods_revenue']-x['old_paid']-total['produced']
  assert x['destination_employer_close']==x['destination_employer_open']+x['retail_goods_revenue']-x['new_paid']
  assert x['retail_goods_revenue']==100*x['retail_units_sold']
  assert x['initial_raw']-x['raw_close']==total['produced']+total['waste']
  assert x['initial_product']+total['produced']-total['sold']==x['product_close']
  assert x['initial_cash']==x['final_cash']
  assert 0<=total['sold']<=112
  out.append({'label':label,'scenario':x['scenario'],'changed':x['changed'],'reference_need_28_days':112,'unfilled_reference_need':112-total['sold'],'reported_unmet_market_demand':total['unmet'],'retail_units_sold':x['retail_units_sold'],'money_stock_work_checks':'passed'})
 if day==10:
  stress_stay,stress_move,solvent_stay,solvent_move,high,low=rows
  assert stress_stay['old_arrears_close']==11500
  assert stress_move['old_arrears_at_switch']==3100 and stress_move['old_arrears_close']==0
  assert (solvent_stay['totals']['produced'],solvent_move['totals']['produced'])==(200,100)
  assert (solvent_stay['totals']['sold'],solvent_move['totals']['sold'])==(112,100)
  def household_demand(new_hourly):
   cash=0; units=0
   for d in range(1,29):
    cash+=8*(20 if d<=10 else new_hourly)
    bought=min(4,cash//102);cash-=bought*102;units+=bought
   return units,cash
  for x,wage in [(high,100),(low,20)]:
   units,cash=household_demand(wage);assert units==x['totals']['sold'] and cash==x['household_close']
summary={'status':'passed','scope':'18 result rows from three fixed-executable runs; independent cash, wage, inventory equations and default affordability oracle','rows':out,'limits':['reference need gap derived for this fixed household need=4/day, not equivalent to reported unmet_market_demand','same complete opening depends also on shared builder and Scenario inheritance; aggregate assertion alone is not a byte identity proof','not endogenous job search, legal labor permissions, sustained financing, or persistence acceptance']}
(r/'employment-independent-validation.json').write_text(json.dumps(summary,indent=2));print('passed18rows; independent default cash-limited demand 87/43; inventory-backed retail actually used in day1 boundary')
