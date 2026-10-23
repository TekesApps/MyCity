import json,sys
from pathlib import Path
p=Path(sys.argv[1]); actual=json.loads(p.read_text())
expected={"scenario":"core_joint_equity_spatial","run_completed":True,"initial_cash_sum":3000000,"final_cash_sum":3000000,"day1_sales":10,"day1_taxable_basis":90,"day1_tax":45,"day2_sales":0,"day2_taxable_basis":0,"day2_tax":0,"day3_sales":10,"day3_taxable_basis":90,"day3_tax":45,"day4_sales":0,"ordinary_dividend_cents":90}
checks={k:{"expected":v,"actual":actual.get(k),"pass":actual.get(k)==v} for k,v in expected.items()}
checks["final_accounts_from_flows"]={"pass":actual["final_cash_accounts_1_to_4"]==[1000000-10000+20+20+90+1,200000-10000+1600-2000-20+89,600000+20000+500-1600-20+2000-90-500-90,1200000-500+500]}
checks["cash_accounts_sum"]={"pass":sum(actual["final_cash_accounts_1_to_4"])==3000000}
checks["store_normal64MiB"]={"pass":actual["final_store_owned_bytes"]<=64*1024*1024}
result={"oracle":"fixed_integer_joint_v1","all_pass":all(x["pass"] for x in checks.values()),"checks":checks,"scope":"microfixture_not_M0_or_real16GB"}
print(json.dumps(result,indent=2));sys.exit(0 if result["all_pass"] else 1)
