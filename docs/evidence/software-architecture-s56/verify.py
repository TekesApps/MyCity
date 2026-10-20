"""Static architecture/source checks only; no execution or performance acceptance."""
from pathlib import Path
import hashlib
import json
import re

ROOT = Path(__file__).resolve().parents[3]
E = Path(__file__).resolve().parent
D = ROOT / 'docs/SOFTWARE_ARCHITECTURE.md'
s = D.read_text()
workspace = (ROOT / 'Cargo.toml').read_text()
assert re.findall(r'crates/city-[a-z]+', re.search(r'^members = (.+)$', workspace, re.M)[1]) == ['crates/city-core', 'crates/city-headless', 'crates/city-storage', 'crates/city-map']
expected = {'city-core':set(), 'city-map':{'mycity-core'}, 'city-storage':{'mycity-core','mycity-map'}, 'city-headless':{'mycity-core','mycity-map','mycity-storage'}}
for crate, names in expected.items():
    t = (ROOT / 'crates' / crate / 'Cargo.toml').read_text().split('[dependencies]')[1].split('[')[0]
    actual = set(re.findall(r'^(mycity-[a-z]+) = ',t,re.M))
    assert actual == names, (crate, actual)
for phrase in ['一个headless进程','不是计算worker池','不是已经完成','纯事件溯源','共享文件是数据交换介质','单一Store writer','规划不等于实际allocator硬门','通用订阅','完整软件尚缺','真实16GB','Sim不是只转发任务的空壳']:
    assert phrase in s, phrase
for name in ['transaction.rs','timeline_driver.rs','session.rs','main.rs']:
    assert name in s
for target in re.findall(r'\[[^\]\n]*\]\(([^)\n]+)\)',s):
    if ':' in target.split('#')[0]:continue
    path=target.split('#')[0]
    assert (D.parent/path).exists(),target
source_names=['.cargo/config.toml','Cargo.toml','Cargo.lock','rust-toolchain.toml']
for directory in ['crates','fixtures']:
    source_names += [str(p.relative_to(ROOT)) for p in (ROOT/directory).rglob('*') if p.is_file()]
h=hashlib.sha256()
for name in sorted(source_names):
    n=name.encode();b=(ROOT/name).read_bytes()
    h.update(len(n).to_bytes(8,'little'));h.update(n)
    h.update(len(b).to_bytes(8,'little'));h.update(b)
base=json.loads((ROOT/'docs/evidence/m0-t05b/verified-summary.json').read_text())
assert h.hexdigest()==base['source_sha256']
assert base['release_tests_passed']==185
assert base['memory_16gb_gate']=='not_run'
readme=(ROOT/'README.md').read_text()
assert '仓库发布版本：`v0.5.22`' in readme
assert 'docs/SOFTWARE_ARCHITECTURE.md' in readme
assert 2*32+32==96 and 192+32==224 and 96+64+8+128==296
report={'release':'v0.5.22','verification':'static_document_and_source_alignment_only','model_baseline':'v0.5.21','model_source_sha256':h.hexdigest(),'rust_or_fixture_changes':False,'crate_dependencies':{k:sorted(v) for k,v in expected.items()},'baseline_release_tests':185,'new_runtime_tests':'not_run_document_only','worker_pool':'not_implemented','independent_sim_host':'not_implemented','unified_subject_event_api':'not_implemented','ui_agentkit_render':'not_implemented','memory_16gb_gate':'not_run','architecture_sha256':hashlib.sha256(D.read_bytes()).hexdigest()}
(E/'checks.json').write_text(json.dumps(report,indent=2)+'\n')
print(json.dumps(report,indent=2))
