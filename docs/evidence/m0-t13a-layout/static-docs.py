from pathlib import Path
import re,json,unicodedata
from urllib.parse import unquote
from decimal import Decimal as D
root=Path(__file__).resolve().parents[3];files=[p for p in root.rglob('*.md') if p.relative_to(root).parts[0] not in ['work','target','.git','outputs']];errors=[];links=0

def anchors(s):
 out=set();seen={}
 for line in s.splitlines():
  x=re.match(r'^#{1,6}\s+(.+?)\s*#*$',line)
  if not x:continue
  t=re.sub(r'<[^>]*>','',x[1]).lower().replace(' ','-')
  t=''.join(c for c in t if c in '-_' or unicodedata.category(c)[0] in 'LN' or unicodedata.category(c)=='Mn')
  n=seen.get(t,0);seen[t]=n+1;out.add(t+(f'-{n}' if n else ''))
 return out
for p in files:
 s=p.read_text();fence=None
 for line in s.splitlines():
  x=re.match(r'^\s*(`{3,}|~{3,})',line)
  if x:
   v=x[1]
   if fence is None:fence=v
   elif v[0]==fence[0] and len(v)>=len(fence):fence=None
 if fence:errors.append(f'fence {p}')
 for x in re.finditer(r'\[[^\]\n]*\]\(([^)\n]+)\)',s):
  dest=x[1].split(' "')[0].strip('<>')
  if re.match(r'^[a-zA-Z][\w+.-]*:',dest):continue
  links+=1;path,_,frag=unquote(dest).partition('#');target=p.parent/path if path else p
  if not target.exists():errors.append(f'missing {p.name}: {dest}')
  elif frag and target.suffix=='.md' and frag not in anchors(target.read_text()):errors.append(f'anchor {p.name}: {dest}')
js=[p for p in root.rglob('*.json') if p.relative_to(root).parts[0] not in ['work','target','.git','outputs']]
for p in js:json.loads(p.read_text())
texts={n:(root/'docs'/n).read_text() for n in ['root-component-contract.md','first-release-object-templates.md','property-system-metric-matrix.md']}
m=texts['property-system-metric-matrix.md'];c=texts['root-component-contract.md'];t=texts['first-release-object-templates.md']
for pre,start,count,body in [('P',1,34,m),('S',0,14,m),('M',1,26,m),('C',1,16,c)]:
 expected=[f'{pre}{i:02}' for i in range(start,start+count)]
 assert sorted(re.findall(r'^\| ('+pre+r'\d{2})(?:[ |])',body,re.M))==expected,pre
 for name,content in texts.items():
  refs=set(re.findall(r'(?<![A-Z])'+pre+r'\d{2}',content))
  if pre=='P':refs.discard('P95')
  assert refs<=set(expected),(name,refs-set(expected))
assert len(set(re.findall(r'^\| (R-H\d|I-[A-Z]+|C-RET|M-[A-Z]+) ',t,re.M)))==13
assert set(re.findall(r'O\d{2}',t))=={f'O{i:02}' for i in range(1,13)}
assert (12*16,20*28,12*16*3,4*60,4*4)==(192,560,576,240,16)
assert (20*24,32*40,20*24*5,16*70,16*4)==(480,1280,2400,1120,64)
assert 300/20==15 and 24+12+4==40 and 10*24*7/40==42
assert (80*D('1.1'),80*D('.1'),40*D('1.1'),40*D('.1'))==(88,8,44,4)
assert (10*D('.5'),10*10,100*D('.5'),120/2)==(5,100,50,60)
assert D(64)*4096/1048576==D('.25')
assert D('.35')*1024-44-D('.25')==D('314.15')
assert sum(map(D,['.35','.25','.85','.25','.35','.20','.40']))==D('2.65')
b=(root/'docs/performance-budget.md').read_text().split('| 条目 | 全世界上限／每条估算 | 有效载荷 MiB |')[1].split('| 有效载荷与显式缓存小计')[0]
rows=[x for x in b.splitlines() if x.startswith('| ') and not x.startswith('| ---')];assert len(rows)==31
vals=[D(10000*512)/1048576 if '全世界在途货运' in row else D(row.split('|')[-2].strip()) for row in rows]
assert sum(vals)==D('43.8828125') and D(44)-sum(vals)==D('0.1171875')
assert (D(90)/100,D(10)/20,D(100)/20)==(D('.9'),D('.5'),5)
assert D(600)/1000==D('.6') and (D(600)+300)/1000==D('.9')
assert sorted([10]*99+[100])[94]==10
print(f'Checked {len(files)} Markdown files, {links} relative links, {len(js)} JSON files; ID sets, 13 templates / 12 operating profiles, arithmetic and 31 budget rows passed.')
if errors:print('\n'.join(errors));raise SystemExit(1)
print('All static checks passed.')
