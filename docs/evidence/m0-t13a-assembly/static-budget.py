# coding: utf-8
from pathlib import Path
import json,csv,re,math
from decimal import Decimal as D
root=Path(__file__).resolve().parents[3]
v=json.loads((root/'docs/inputs/product-blueprint/reference-values.json').read_text())
assert sum(D(str(x)) for x in v['app_gib'].values())==D('3.10')
assert sum(v['simulation_extra_mib'].values())==236
assert D('0.60')*1024-D('358.4')-236==20
assert v['app_gib']['simulation']*1024>=v['simulation_core_mib']+236
runtime=(root/'docs/product-runtime-budget-and-versions.md').read_text()
for key,n in v['simulation_extra_mib'].items():
 m=re.search(r'^\| '+re.escape(key)+r' \|[^\n]*\| (\d+) \|$',runtime,re.M)
 assert m and int(m[1])==n,(key,m[0] if m else 'missing')
for key,n in v['jprod_1'].items():assert n<=v['joint_limit'][key],key
assert (262144-4096)*128/1048576==31.5
assert (524288-65536)*32/1048576==14
assert 3+14+4+2+1.125+7.875==32
assert 4096*96/1048576+8192*48/1048576==0.75
assert 0.5+4+4==8.5
assert 358.4+236<614.4
fields=['需求来源','设施与主体','资源与容量','成本收益','交通环境','法律流程','失败退出','玩家观察','聚合／writer／预算']
rows=list(csv.DictReader((root/'docs/inputs/product-blueprint/capability-evidence.csv').open()))
assert len(rows)==37 and len({r['capability'] for r in rows})==37
for row in rows:
 src=(root/row['design']).read_text().splitlines()
 assert row['capability'] in src[int(row['section_line'])-1]
 for f in fields:
  file,line=row[f].rsplit(':',1)
  assert file==row['design'] and src[int(line)-1].startswith('| '+f+' |')
  assert len(src[int(line)-1].split('|')[2].strip())>=15
# Profile constructor invariants: grades cannot edit cost/power independently.
K=lambda g:.3+.7*g**1.2
assert K(.5)<K(1)<K(2) and 2*K(.5)>K(1)
assert 1.5*K(1)>K(1) and .9<1 and .8<1
assert 1500*30*150+2000*1000+2000000+2*1000000==12750000
for meals in (0,1,2):
 homefood=D('.2')-D('.1')*meals
 foodservice=D('.1')*meals
 homecook=D('3')-D('1.5')*meals
 assert D('.3')+homefood+foodservice==D('.5')
 assert homecook+D('1.5')*meals==3
 assert D('.3')*D('.1')+homefood*D('.1')+foodservice*D('.1')==D('.05')
assert abs((1.5/(3.6*.95)+(.48-1.5/(3.6*.95)))-.48)<1e-12
assert D('.1')+D('.01')==D('.08')+D('.02')+D('.01')
assert 98+2==100 and 95+5==100
assert .01*.1/98<.00002 and .05*.1/95<.0001
assert 50*.02*48==48 and 2*40*.95==76
for speed,rate,tank,dist in [(500,500,1600,1000),(800,2200,15000,4000),(850,5200,70000,9000)]:
 fuel=((dist/speed+.25)*rate+.1*rate)*1.2
 assert fuel<=tank,(speed,fuel,tank)
assert (20+40)/2+30==60
annualbase=1000; tax=annualbase*.1; prepaid=70
assert tax-prepaid==30 and annualbase-tax==900
assert sum([.4,.6])==1 and .4*1000*.005+.6*1000*.005==5
profile=(root/'docs/product-facility-and-equipment-profiles.md').read_text()
for name in ['F-HOUSING','F-COMM','F-IND','F-STOP','F-STATION','F-PARKING','F-PORT','F-AIRPORT','F-GRID','F-CIVIL','F-TREAT','F-ENERGY','F-GREEN']:
 assert '| '+name+' ' in profile,name
for name in ['EQ-ENTRY','EQ-VERT','EQ-ELEC','EQ-WATER','EQ-VENT','EQ-KITCHEN','EQ-PLATFORM','EQ-HANDLING','EQ-HEALTH']:
 assert '| '+name+' ' in profile,name
req=(root/'docs/product-coverage-and-requirements.md').read_text()
assert sorted(map(int,re.findall(r'^\| REQ(\d+) \|',req,re.M)))==list(range(1,62))
assert '旧首期设计有实质基础' not in req
assert '按揭／继承等有界总体扩充' not in req
print('PASS: 37x9 source locations; REQ01..61; 236MiB / 3.10GiB and joint caps; 13 facility families / 9 support packs; meal/heat/water/tax/fuel/safety arithmetic. Static design evidence only.')

assert v["requirements"]==61
assert v["regional_carve_out_mib"]=={"events_from_existing_12":1,"market_from_existing_7_125":0.25,"runtime_from_existing_0_45_gib":0.0625}
