from pathlib import Path
import hashlib,json,re,sys
B=Path(__file__).resolve().parent
checks={};details={}
final=json.loads((B/'147-compensation-joint-build/source-manifest.json').read_text())
expected={'142-compensation-storage':[21,4,2,2,1,13,5], '144-compensation-final-libraries':[142,93], '145-compensation-final-engine':[16]}
for label,counts in expected.items():
    p=B/label;e=json.loads((p/'exit.json').read_text());log=(p/'stdout.log').read_text()
    observed=[int(x) for x in re.findall(r'test result: ok\. (\d+) passed;',log)]
    checks[label]=e['exit_code']==0 and e['source_unchanged'] and observed==counts
    details[label]={'passed_counts':observed,'expected_counts':counts}
for label in list(expected)+['143-compensation-strict','146-compensation-fmt','147-compensation-joint-build','148-compensation-joint-cases']:
    p=B/label;e=json.loads((p/'exit.json').read_text())
    checks[label+'_final_source']=json.loads((p/'source-manifest.json').read_text())==final and e['exit_code']==0 and e['source_unchanged']
rows=json.loads((B/'148-compensation-joint-cases/cases/results.json').read_text())
checks['seven_cli_cases']=len(rows)==7 and all(x['exit_code']==0 for x in rows)
for row in rows:
    name=row['case'];a=B/'120-employment-lifelines-freight-cases/cases'/name/'stdout.log';b=B/'148-compensation-joint-cases/cases'/name/'stdout.log'
    checks['legacy_cli_unchanged_'+name]=a.read_bytes()==b.read_bytes()
    details[name]={'stdout_sha256':hashlib.sha256(b.read_bytes()).hexdigest(),'state_hash':re.search(r'state_hash=([a-f0-9]+)',b.read_text()).group(1)}
worlds=list((B/'142-compensation-storage/test-artifacts').glob('mycity-construction-compensation-*/world'))
checks['complete_compensation_original_worlds']=len(worlds)==2 and all((p/'genesis.json').exists() and (p/'CURRENT.json').exists() for p in worlds)
p=next(p for p in worlds if (p/'checkpoint-7.json').exists())
g=json.loads((p/'genesis.json').read_text());opening=json.loads(bytes(g['opening']));cp=json.loads((p/'checkpoint-7.json').read_text());engine=json.loads(bytes(cp['engine']));w=engine['world'];project=w['construction']
old={x['id']:x['cash'] for x in opening['accounts']};new={x['value']['id']:x['value']['cash'] for x in w['accounts']['slots'] if x['value'] is not None}
checks['checkpoint_exact_transfer']=new=={**old,4:old[4]-50000,2:old[2]+50000}
checks['checkpoint_money_conserved']=sum(old.values())==sum(new.values())
checks['checkpoint_paid_marker_and_spent']=project['spent']==50000 and project['compensation']['settled_operation_id']==700006 and project['compensation']['amount']==50000 and project['review_round']==1
checks['checkpoint_gate_matches_actual_payment']=project['gates'][2]['decided']=='accepted' and project['gates'][2]['party']==2
paid=[x for x in cp['receipts'] if x['operation_id']==700006]
checks['checkpoint_single_success_receipt']=len(paid)==1 and 'Ok' in paid[0]['outcome'] and paid[0]['actor']==2
report={'status':'passed' if all(checks.values()) else 'failed','checks':checks,'details':details,'total_distinct_final_tests':299,'scope':'output arithmetic, identical final fixed inputs, complete retained originals and CLI legacy identity; not performance or calibrated balance','storage':'external HP P900 USB/APFS SSD'}
(B/'compensation-independent.json').write_text(json.dumps(report,indent=2)+'\n')
print(json.dumps(report,indent=2));sys.exit(0 if all(checks.values()) else 1)
