"""Read-only artifact consistency audit; does not run the game or tests."""
import hashlib
import json
from pathlib import Path
import subprocess

REPO = Path(__file__).resolve().parents[3]
PHASE = REPO / 'work/task-registry-directory-20261026'
sha = lambda p: hashlib.sha256(p.read_bytes()).hexdigest()
load = lambda p: json.loads(p.read_text())

def verify():
    inputs = load(PHASE / 'source07-build-inputs.json')
    for entry in inputs:
        assert sha(REPO / entry['file']) == entry['sha256'], entry['file']
    wip = load(PHASE / 'preserved-wip.json')
    for name, entry in wip.items():
        assert sha(REPO / name) == entry['sha256'], name
    results = load(PHASE / 'source08-results.json')
    assert len(results) == 6
    current_main = sha(PHASE / 'source07-headless-fixed')
    for r in results:
        assert r['exit_code'] == 0 and r['main_sha256'] == current_main
        assert sha(Path(r['command'][0])) == r['test_sha256']
        assert 'test result: ok.' in (PHASE / ('source08-' + r['target'] + '.stdout')).read_text()
    r = load(PHASE / 'source07-result.json')
    assert r['exit_code'] == 0 and r['headless_sha256'] == current_main
    assert sha(Path(r['command'][0])) == r['test_sha256']
    assert '9 passed; 0 failed' in (PHASE / 'source07.stdout').read_text()
    r = load(PHASE / 'source06-component-result.json')
    assert r['exit_code'] == 0 and sha(Path(r['command'][0])) == r['sha256']
    assert '13 passed; 0 failed' in (PHASE / 'source06-component.stdout').read_text()
    r = load(PHASE / 'source10-result.json')
    assert r['exit_code'] == 0 and r['headless_sha256'] == current_main
    assert sha(Path(r['command'][0])) == r['fixed_sha256']
    loan = load(PHASE / 'normal-loan10/actual-loan-proof.json')
    assert loan['principal_cents'] == 500 and loan['final_cash'] == [201000, 599000]
    assert loan['original_regional_obligation']['paid_cents'] == 1000
    assert all(loan[k] for k in ['loan_and_C_retry_once','original_manual_reference_equal','cold_sim_no_new_future_target','not_pressure_proxy'])
    roles = [load(PHASE / n) for n in ['source07-joint-role-proof.json','source08-v2-role-proof.json']]
    for r in roles:
        for o in r['observations']:
            assert o['allocation_failures'] == 0
            assert o['lifetime_peak_requested_bytes'] <= 59 * 1024 * 1024
        pool = r.get('full_pool_return', r.get('pool_return'))
        assert pool['available_touched_bytes'] == pool['capacity_bytes'] == 64 * 1024 * 1024
        assert pool['workspace_grants'] == pool['workspace_returns']
        assert pool['workspace_capacity_bytes'] == pool['granted_capacity_bytes'] == 0
    roots = [PHASE.resolve()] + [(REPO / 'work' / n).resolve() for n in ['r35','r36','r37','r38']]
    size = sum(f.stat().st_size for root in roots for f in root.rglob('*') if f.is_file())
    assert size <= 512 * 1024 * 1024, size
    assert subprocess.check_output(['git','diff','--name-only','HEAD','--','crates/city-core','crates/city-storage','Cargo.toml','Cargo.lock'],cwd=REPO) == b''
    return {'audit_kind':'artifact consistency, not a new game test','source_inputs_verified':len(inputs),'preserved_wip':len(wip),'current_new_tests':9,'affected_groups':6,'pure_component_tests':13,'normal_loan_and_regional_due':loan,'current_main_sha256':current_main,'complete_roots':[str(r) for r in roots],'logical_bytes':size,'disk_budget_bytes':512*1024*1024,'not_proved':['native whole-process peak','whole-game/GPU shared budget','real 16GB','M0 frozen gate','sustainable speed','future cache provider']}

if __name__ == '__main__':
    print(json.dumps(verify(), ensure_ascii=False, indent=2))
