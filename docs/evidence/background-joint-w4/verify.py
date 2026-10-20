#!/usr/bin/env python3
"""Verify frozen W4 artifacts, complete business/air state, and timed group samples.

Default verifies published run.zip and its manifest. --run DIR verifies an original
run directory. --dry explicitly permits a short developer run; it never grants
endurance acceptance. Source is required so a later tag cannot silently reuse it.
"""
import argparse
import copy
import hashlib
import json
import re
import struct
import zipfile
from pathlib import Path

HERE = Path(__file__).resolve().parent
ROOT = HERE.parents[2]
SOURCE = '829b88384cd243bdb66f4c4c9deff59c764067412d5dc2c856bef21149cf7417'


def require(ok, why):
    if not ok:
        raise SystemExit(why)


def sha(raw):
    return hashlib.sha256(raw).hexdigest()


def canonical(value):
    return json.dumps(value, sort_keys=True, separators=(',', ':'), ensure_ascii=False).encode()


def source_hash():
    names = ['.cargo/config.toml', 'Cargo.toml', 'Cargo.lock', 'rust-toolchain.toml']
    names += [p.relative_to(ROOT).as_posix() for folder in ['crates', 'fixtures']
              for p in (ROOT / folder).rglob('*') if p.is_file()]
    h = hashlib.sha256()
    for name in sorted(names):
        b, raw = name.encode(), (ROOT / name).read_bytes()
        h.update(struct.pack('<Q', len(b)) + b + struct.pack('<Q', len(raw)) + raw)
    return h.hexdigest()


def business(engine):
    result = {k: copy.deepcopy(engine[k]) for k in ['world', 'resources', 'leases', 'next_token', 'timeline']}
    result['world']['revision'] = 0
    result['schema'] = 'mycity-business-comparison-v1'
    return result


def slots(arena):
    return [s['value'] for s in arena['slots'] if s['value'] is not None]


def check(read, dry):
    r = json.loads(read('report.json'))
    policy_raw = (ROOT / 'fixtures/m0-background-joint-policy.json').read_bytes()
    opening_raw = (ROOT / 'fixtures/m0-background-joint-opening.json').read_bytes()
    p, opening = json.loads(policy_raw), json.loads(opening_raw)
    require(r['source_sha256'] == SOURCE and r['policy_sha256'] == sha(policy_raw)
            and r['opening_sha256'] == sha(opening_raw), 'source/input provenance')
    require(r['game_seconds'] == p['game_seconds'] == 604800, 'seven game days absent')
    if dry:
        require(r['requested_wall_seconds'] == 0 and r['endurance_1800_seconds'] == 'not_run_dry', 'dry scope')
    else:
        require(r['requested_wall_seconds'] >= p['minimum_wall_seconds'] == 1800
                and r['work_wall_seconds'] >= 1800 and r['endurance_1800_seconds'] == 'passed', 'actual endurance absent')
    require(r['memory_16gb_gate'] == 'not_run' and r['full_M0'] == 'not_accepted'
            and r['independent_physical_accuracy'] == 'not_verified', 'scope overclaim')
    require(r['configured_limits'] == {'sim_reader_frames': 4, 'worker_reader_frames': 4,
            'control_bytes': 65536, 'inflight_jobs': 1, 'attempts_per_job': 3,
            'queue_peak_occupancy': 'not_instrumented'}, 'bounded protocol scope')
    checkpoints = r['checkpoints']
    require([c['game_seconds'] for c in checkpoints] == [60, 120] + [h * 3600 for h in range(1, 169)], 'missing comparison checkpoints')
    for c in checkpoints:
        require(c['business_sha256'] == c['reference_business_sha256'] == c['reference']['business_sha256'], 'business comparison differs')
        require(c['air_sha256'] == c['reference_air_sha256'] == c['reference']['air_sha256'], 'complete air comparison differs')
        require(c['reference']['cash_total'] == 3000000, 'cash not conserved')
        require(c['capacity']['worker_cache_actual_bytes'] <= 6 * 1024**2, 'cache cap')
        if not dry and c['game_seconds'] >= 3600:
            due = c['game_seconds'] / 604800 * r['requested_wall_seconds']
            require(c['elapsed_seconds'] - r['work_started_elapsed_seconds'] >= due, 'endurance padded after game completion')
    times = [c['elapsed_seconds'] for c in checkpoints]
    require(all(a < b for a, b in zip(times, times[1:])), 'checkpoint clock order')

    raw = read('transcript.jsonl')
    require(len(raw) == r['transcript_bytes'] <= 32 * 1024**2, 'transcript size')
    lines = raw.splitlines()
    require(all(len(line) <= 128 * 1024 for line in lines), 'transcript line bound')
    transcript = [json.loads(line) for line in lines]
    require(all(a['elapsed_seconds'] <= b['elapsed_seconds'] for a, b in zip(transcript, transcript[1:])), 'transcript clock order')
    events = [x['data']['event'] for x in transcript if 'event' in x['data']]
    raw_checkpoints = [x['data']['verified_checkpoint'] for x in transcript if 'verified_checkpoint' in x['data']]
    require(raw_checkpoints == checkpoints, 'report checkpoints differ from raw transcript')
    fault_commands = [x['data'] for x in transcript if 'fault' in x['data']]
    require([x['fault'] for x in fault_commands] == ['worker_sigkill', 'sim_sigkill_committed_before_reply'], 'raw fault actions absent')
    daily = [x['data']['daily_progress'] for x in transcript if 'daily_progress' in x['data']]
    require([d['hour'] for d in daily] == list(range(24, 169, 24)), 'daily continued progress absent')
    ready = [e for e in events if e['kind'] == 'ready']
    require(len(ready) == 3 and len({e['sim_pid'] for e in ready}) == 3, 'Sim crash/reopen lifecycle')
    require(all(e['payload']['source_sha256'] == SOURCE for e in ready), 'Sim executable source')
    faults = r['faults']
    require(len(faults) == 2 and faults[0]['kind'] == 'worker_sigkill'
            and faults[1]['kind'] == 'sim_sigkill_committed_before_reply'
            and faults[0]['job'] != faults[1]['job'], 'two distinct actual fault cases')
    require(faults[0]['barrier']['details']['cursor'] > 0 and r['attempt_failures'] == 1, 'mid-page failure absent')
    started = [e['payload'] for e in events if e['kind'] == 'worker_started']
    retry = [s for s in started if s['ticket']['job'] == faults[0]['job']]
    require([s['ticket']['generation'] for s in retry] == [1, 2]
            and retry[0]['worker_pid'] != retry[1]['worker_pid'], 'worker retry generation')
    f = faults[1]
    require(f['before']['timeline']['at'] >= 86400 and f['result']['replayed'], 'post-business committed reply fault absent')
    for k in ['accounts', 'payroll', 'obligations']:
        require(f['before']['runtime'][k] == f['reopened']['runtime'][k], 'Sim recovery duplicated business ' + k)
    require(f['before']['business_sha256'] == f['reopened']['business_sha256'], 'full business recovery mismatch')
    require(f['before']['runtime']['payroll'][0]['paid'] == 56
            and f['before']['runtime']['accounts'][0]['cash'] == 1200000, 'fault did not follow actual wage/tax payment')
    barriers = [e['payload'] for e in events if e['kind'] == 'probe_barrier']
    require([b['point'] for b in barriers] == ['worker_cells', 'committed'], 'raw fault barriers')
    require([b for b in barriers] == [f['barrier'] for f in faults], 'fault report/transcript differ')

    raw_samples = read('group-samples.jsonl')
    rows = [json.loads(line) for line in raw_samples.splitlines()]
    stats = r['resource_samples']
    require(len(rows) == stats['snapshots'] <= 12000 and len(raw_samples) == stats['log_bytes'] <= 64 * 1024**2, 'sample bounded count')
    require(all(len(line) <= 128 * 1024 for line in raw_samples.splitlines()), 'sample line bound')
    for row in rows:
        require(row['sum_rss_bytes'] == sum(p['rss_bytes'] for p in row['processes']), 'RSS not from same snapshot')
        require(len({p['pid'] for p in row['processes']}) == len(row['processes']), 'double PID counting')
    require(all(a['elapsed_seconds'] < b['elapsed_seconds'] and a['unix_ms'] <= b['unix_ms']
                for a, b in zip(rows, rows[1:])), 'sample clock order')
    peak = max(rows, key=lambda x: x['sum_rss_bytes'])
    require((peak['sum_rss_bytes'], peak['elapsed_seconds'], peak['processes']) ==
            (stats['peak_sum_rss_bytes'], stats['peak_at_seconds'], stats['peak_processes']), 'RSS peak receipt differs')
    gap = max(b['elapsed_seconds'] - a['elapsed_seconds'] for a, b in zip(rows, rows[1:]))
    require(abs(gap - stats['maximum_gap_seconds']) < 1e-8, 'sampling gaps hidden')
    require(max(x['world_actual_file_bytes'] for x in rows) == stats['peak_disk_bytes']
            and max(x['experiment_actual_file_bytes'] for x in rows) == stats['peak_experiment_bytes'], 'observed disk peaks')
    require(max(len(x['processes']) for x in rows) == stats['maximum_group_processes'], 'group size')
    roles = {p['role'] for x in rows for p in x['processes']}
    require({'host_cpu_reference_and_sampler', 'sim', 'worker', 'sampler_ps'} <= roles, 'whole process roles missing')
    require(rows[-1]['processes'] == stats['last_group'] and
            all(p['role'] in ['host_cpu_reference_and_sampler', 'sampler_ps'] for p in stats['last_group']), 'remaining child/orphan process')
    observed = {p['pid'] for x in rows for p in x['processes']}
    declared = {e['sim_pid'] for e in ready} | {s['worker_pid'] for s in started}
    require(sorted(observed) == stats['observed_pids'] and sorted(declared - observed) == stats['unobserved_declared_pids'], 'PID sampling disclosure')
    require({faults[0]['pid'], f['old_sim_pid'], f['new_sim_pid']} <= observed, 'fault process not measured')
    require(rows[0]['elapsed_seconds'] < times[0] and rows[-1]['elapsed_seconds'] > times[-1], 'sampling does not cover workload')
    if not dry:
        require(rows[-1]['elapsed_seconds'] - rows[0]['elapsed_seconds'] >= 1800
                and rows[-1]['unix_ms'] - rows[0]['unix_ms'] >= 1800000, 'actual timed samples missing')

    current = json.loads(read('world/CURRENT.json'))
    root_raw = read(f"world/root-{current['slot']}.json")
    require(sha(root_raw) == current['root_sha256'], 'CURRENT root hash')
    root = json.loads(root_raw)
    require(root['wal_end'] == 0 and root['continuation'] is None
            and root['background_reserve'] is None and root['worker_cache_bytes'] == 6 * 1024**2, 'final durable boundary/cache funding')
    genesis = read('world/genesis.json')
    require(sha(genesis) == root['genesis_sha256'] and json.loads(genesis)['rules_identity'] == 'sim-w4:' + SOURCE, 'Store provenance')
    ref = root['checkpoint']
    all_receipts = {}
    latest = None
    generations = set()
    while ref is not None:
        generation = ref['generation']
        require(generation not in generations and len(generations) < 64, 'checkpoint chain cycle/cap')
        generations.add(generation)
        raw = read(f'world/checkpoint-index-{generation}.json')
        require(sha(raw) == ref['sha256'], 'checkpoint index hash')
        index = json.loads(raw)
        raw = read(f'world/checkpoint-{generation}.json')
        require(sha(raw) == index['data_sha256'], 'checkpoint data hash')
        cp = json.loads(raw)
        require(cp['previous'] == index['previous'] and [v['operation_id'] for v in cp['receipts']] == index['operations'], 'checkpoint receipt chain')
        for record in cp['receipts']:
            require(record['operation_id'] not in all_receipts, 'duplicate durable operation receipt')
            all_receipts[record['operation_id']] = record
        if latest is None:
            latest = cp
            require(cp['sequence'] == root['sequence'] and cp['chain'] == root['wal_chain']
                    and cp['genesis_sha256'] == root['genesis_sha256'], 'root checkpoint binding')
        ref = index['previous']
    e = json.loads(bytes(latest['engine']))
    reference = json.loads(read('reference-engine.json'))
    require(business(e) == business(reference), 'entire committed business differs from direct CPU reference')
    require(sha(canonical(business(e))) == r['reference']['business_sha256'], 'reported business digest')
    require(r['final_state']['business_sha256'] == ready[-1]['payload']['business_sha256'] == r['reference']['business_sha256'], 'final reopened business status')
    bg = e['background']
    require(bg['pending'] is None and bg['version'] == r['air_windows'] == 173
            and bg['field'] == reference['background']['field'], 'entire durable air state differs')
    air_sha = r['reference']['air_sha256']
    require(r['final_state']['background']['state_sha256'] == ready[-1]['payload']['background']['state_sha256'] == air_sha, 'final reopened air status')
    candidate_raw = read('world/air-worker/candidates/' + air_sha + '.air')
    require(sha(candidate_raw) == air_sha and json.loads(candidate_raw)['payload'] == bg['field'], 'full CPU air candidate mismatch')
    final_receipt = r['final_original_field_receipt']
    require(final_receipt['replayed'] and all_receipts[0xe000000000000000 | 173] == final_receipt['record'], 'cold final receipt not original')
    require(all_receipts[0xe000000000000000 | f['job']] == f['result']['record'], 'lost reply not durable original')
    loan = r['loan_envelope']
    require(all_receipts[loan['operation_id']]['outcome'].get('Ok') is not None
            and all_receipts[loan['operation_id']]['actions'][0] == loan['actions'][0], 'atomic loan not durably admitted')

    world = e['world']
    accounts = slots(world['accounts'])
    require([a['cash'] for a in accounts] == [2400000, 196950, 47636, 354550, 336, 528]
            and sum(a['cash'] for a in accounts) == 3000000 and all(a['protected'] == 0 for a in accounts), 'normative cash/wages/tax')
    tax = p['tax']
    require(tax['kind'] == 'fixed_daily_business_tax' and tax['period_seconds'] == 86400, 'tax policy changed')
    opening_obligation = opening['region']['obligations'][0]
    require(opening_obligation == {'id': tax['obligation_id'], 'payer': tax['payer'],
            'payee': tax['treasury'], 'amount': tax['amount_cents'],
            'next_due': tax['first_due'], 'period_seconds': tax['period_seconds'],
            'expires_at': tax['ends_at'], 'arrears': tax['opening_arrears_cents']}, 'tax rule not mapped to actual opening obligation')
    due_count = (tax['ends_at'] - tax['first_due']) // tax['period_seconds'] + 1
    require(due_count * tax['amount_cents'] == accounts[0]['cash'] - 1000000 == 1400000, 'actual Treasury tax cash')
    obligation = world['region']['obligations'][0]
    require(obligation['arrears'] == 0 and obligation['next_due'] is None and r['tax_arrears_observed_cents'] == 201612, 'tax shortage/settlement absent')
    require(e['timeline']['progress']['last_complete'] == 604800 and e['timeline']['progress']['phase'] is None, 'timeline incomplete')
    require([x['paid'] for x in e['timeline']['payroll']] == [336, 528]
            and all(x['work_seconds'] == 6 * 8 * 3600 for x in e['timeline']['payroll']), 'inactive city wage double run')
    facts = r['realized_facts']
    require(facts['boot'] == world['boot'] and facts['boot']['fuel_used'] == 1000 and facts['boot']['fees'] == 500
            and all(j['completed'] == 1 and j['failed'] == 0 and j['phase'] == 'Idle' for j in facts['boot']['jobs']), 'actual startup completion')
    expected_stocks = [4000, 400000, 540000, 5400000, 60000, 700000, 3000, 1000, 1000] + [0] * 5
    require([s['quantity'] for s in slots(world['stocks'])] == expected_stocks, 'resource inputs/outputs not realized')
    for q in world['exchange']['quotes'][:2]:
        require(q['status'] == 'Delivered' and (q['delivered'], q['invoice'], q['freight'], q['source_cost']) == (1000, 2000, 20, 400), 'actual freight not settled')
    require(facts['delivered_quotes'] == world['exchange']['quotes'][:2], 'freight facts not actual')
    require(facts['resources'] == e['resources'] and all(x['reserved'] == x['occupied'] == 0 for x in e['resources'])
            and all(x is None for x in e['leases']) and e['next_token'] == 9, 'shared capacity leaked')
    inbox = world['exchange']['inbox']
    require(inbox == facts['inbox'] and inbox['accrued'] == inbox['paid'] == 10 and inbox['closed'] == 2
            and all(x is None for x in inbox['facts'] + inbox['retained']), 'inbox responsibility not settled')
    claims = e['timeline']['claims']
    require(claims[0] == facts['paid_contract'] and claims[0]['paid'] == 1000 and claims[0]['processed'], 'contract debt not paid')
    require(claims[1] == facts['future_loan'] and claims[1]['principal'] == 850000 and claims[1]['paid'] == 0
            and claims[1]['due'] == 864000 and not claims[1]['processed'], 'future loan falsely revenue or repaid')
    return {'source_sha256': SOURCE, 'mode': 'dry' if dry else 'endurance',
            'endurance_1800_seconds': 'not_run_dry' if dry else 'passed',
            'work_wall_seconds': r['work_wall_seconds'], 'comparison_checkpoints': len(checkpoints),
            'air_windows': r['air_windows'], 'actual_faults': len(faults),
            'complete_durable_business_and_air_match': True, 'checkpoint_archives': len(generations),
            'durable_receipts': len(all_receipts), 'snapshots': len(rows),
            'observed_peak_sum_rss_bytes': stats['peak_sum_rss_bytes'],
            'maximum_sampling_gap_seconds': gap, 'unobserved_declared_pids': sorted(declared - observed),
            'memory_16gb_gate': 'not_run', 'full_M0': 'not_accepted',
            'rss_limit': 'same-snapshot RSS sums; shared pages may double count; between-sample peaks may be missed'}


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--run', type=Path)
    parser.add_argument('--dry', action='store_true')
    args = parser.parse_args()
    require(source_hash() == SOURCE, 'use frozen W4 source/tag')
    if args.run:
        result = check(lambda name: (args.run / name).read_bytes(), args.dry)
    else:
        require(not args.dry, 'published evidence must be endurance')
        manifest = json.loads((HERE / 'artifact-manifest.json').read_bytes())
        with zipfile.ZipFile(HERE / 'run.zip') as archive:
            require(set(archive.namelist()) == set(manifest) and len(archive.namelist()) <= 2048, 'archive inventory')
            require(sum(v.file_size for v in archive.infolist()) <= 192 * 1024**2, 'archive byte cap')
            for name, entry in manifest.items():
                require(not Path(name).is_absolute() and '..' not in Path(name).parts, 'unsafe archive name')
                raw = archive.read(name)
                require(len(raw) == entry['bytes'] and sha(raw) == entry['sha256'], 'artifact checksum: ' + name)
            require(archive.read('report.json') == (HERE / 'probe.json').read_bytes(), 'published report copy differs from archive')
            result = check(archive.read, False)
        launch = json.loads((HERE / 'launch.json').read_bytes())
        require(launch['source_sha256'] == SOURCE and launch.get('exit_code') == 0
                and launch['command_wall_seconds'] >= 1800 and launch['command'][3] == '1800', 'formal launch/exit receipt')
        gates = json.loads((HERE / 'gates.json').read_bytes())
        require({g['gate'] for g in gates} == {'fmt', 'clippy', 'release-tests'}
                and all(g['exit_code'] == 0 for g in gates), 'release gates incomplete')
        log = (HERE / 'release-tests.txt').read_text()
        require('test result: FAILED' not in log, 'release test failure')
        result['release_tests_passed'] = sum(map(int, re.findall(r'test result: ok\. (\d+) passed;', log)))
        result['run_zip_sha256'] = sha((HERE / 'run.zip').read_bytes())
        result['verifier_sha256'] = sha(Path(__file__).read_bytes())
        (HERE / 'checks.json').write_text(json.dumps(result, indent=2) + '\n')
    print(json.dumps(result, indent=2))


if __name__ == '__main__':
    main()
