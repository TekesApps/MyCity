"""Static tree/climate design consistency; no ecological or runtime acceptance."""
from pathlib import Path
import hashlib,json,re,subprocess
ROOT=Path(__file__).resolve().parents[3];E=Path(__file__).resolve().parent
s=(ROOT/'docs/tree-species-climate-and-air.md').read_text()
for phrase in ['灰度图不包含经纬度','条件适生','不因气候标签改变立即删树','0.01h','48KiB','5.953125','EI是游戏指数','当前T12-A','M-env','unsupported']:
    assert phrase in s,phrase
assert 4096*4==64*256==256*64==16384
assert (4096*4+64*256+256*64)/1048576==0.046875
assert 6-0.046875==5.953125
assert '仓库发布版本：`v0.5.24`' in (ROOT/'README.md').read_text()
req=(ROOT/'docs/product-coverage-and-requirements.md').read_text()
assert len(re.findall(r'^\| REQ\d+ \|',req,re.M))==61
assert '| REQ23 | 植被生长／养护、树种差异与地理适生' in req
names=['.cargo/config.toml','Cargo.toml','Cargo.lock','rust-toolchain.toml']
for d in ['crates','fixtures']:names += [str(p.relative_to(ROOT)) for p in (ROOT/d).rglob('*') if p.is_file()]
h=hashlib.sha256()
for name in sorted(names):
 n=name.encode();b=(ROOT/name).read_bytes();h.update(len(n).to_bytes(8,'little'));h.update(n);h.update(len(b).to_bytes(8,'little'));h.update(b)
base=json.loads((ROOT/'docs/evidence/m0-air-cpu/verified-summary.json').read_text())
assert h.hexdigest()==base['source_sha256'] and base['release_tests_passed']==195
assert not subprocess.check_output(['git','diff','--name-only','--','crates','fixtures','Cargo.toml','Cargo.lock','rust-toolchain.toml','.cargo/config.toml'],cwd=ROOT)
r={'release':'v0.5.24','scope':'static_design_only','source_sha256_unchanged':h.hexdigest(),'requirement':'REQ23','planned_increment_bytes':49152,'remaining_m_env_subpool_mib':5.953125,'existing_plant_slots':4096,'tree_species_runtime':'not_implemented','georeferencing_runtime':'not_implemented','tree_air_runtime':'not_implemented','memory_16gb_gate':'not_run','document_sha256':hashlib.sha256(s.encode()).hexdigest(),'sources':['https://planthardiness.ars.usda.gov/pages/how-to-use-the-maps','https://www.forestresearch.gov.uk/tools-and-resources/fthr/ecological-site-classification/','https://www.forestresearch.gov.uk/climate-change/advice/urban-forests/']}
(E/'checks.json').write_text(json.dumps(r,indent=2)+'\n');print(json.dumps(r,indent=2))
