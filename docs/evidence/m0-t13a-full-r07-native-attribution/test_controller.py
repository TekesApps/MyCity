"""Pure external diagnostic protocol tests. No model/process is launched."""
import ast
import copy
import json
from pathlib import Path
import tempfile
import time
import unittest

HERE = Path(__file__).resolve().parent
SOURCE = '32c6e61b9565545177c330f34455611b329fbe1cb38591ec5f6e7879731c4da2'

def functions(environment):
    tree = ast.parse((HERE / 'attempt03/controller.py').read_text())
    nodes = [node for node in tree.body if isinstance(node, ast.FunctionDef)
             and node.name in ('advance_protocol', 'full_repaid_frame')]
    assert len(nodes) == 2
    exec(compile(ast.Module(body=nodes, type_ignores=[]), 'actual-controller-functions', 'exec'), environment)
    return environment

class ProtocolTests(unittest.TestCase):
    def test_explicit_first_second_pause_markers_and_idempotence(self):
        with tempfile.TemporaryDirectory(prefix='mycity-marker-pure-') as temporary:
            root = Path(temporary)
            layout = root / 'model/layout'
            layout.mkdir(parents=True)
            env = functions(dict(work=root, observed=root, layout=layout,
                pause=root/'r07-provider-pause', protocol=[], json=json, time=time))
            frame = {'group_capture_complete': True, 't': 2,
                     'r07_applied': {'state': {'replaced_bytes': 1, 'epoch': 1}}}
            events = layout/'events.jsonl'
            events.write_text(json.dumps({'scan': {'iteration': 1}})+'\n')
            env['advance_protocol'](frame)
            self.assertEqual(env['protocol'], [])
            events.write_text(json.dumps({'scan': {'iteration': 2}})+'\n')
            env['advance_protocol'](frame)
            self.assertEqual([row['name'] for row in env['protocol']], ['first'])
            with events.open('a') as out:
                out.write(json.dumps({'snapshot_durable': {'generation': 1}})+'\n')
            env['advance_protocol'](frame)
            self.assertEqual([row['name'] for row in env['protocol']], ['first','second'])
            with events.open('a') as out:
                out.write(json.dumps({'snapshot_admitted': {'generation': 2}})+'\n')
            env['advance_protocol'](frame)
            env['advance_protocol'](frame)
            self.assertEqual([row['name'] for row in env['protocol']], ['first','second','pause'])
            for name in ['vm-start-first', 'vm-start-second']:
                self.assertTrue((layout/name).is_file())
            self.assertTrue((root/'r07-provider-pause').is_file())

    def test_full_repayment_is_not_a_fixed_epoch_number(self):
        rows = [json.loads(row) for row in (HERE/'attempt02/observed/host-samples.jsonl').read_text().splitlines()]
        env = functions({})
        positive = next(row for row in rows if env['full_repaid_frame'](row, SOURCE))
        self.assertEqual(positive['r07_applied']['state']['epoch'], 2)
        self.assertTrue(env['full_repaid_frame'](positive, SOURCE))
        different_version = copy.deepcopy(positive)
        different_version['r07_applied']['state']['epoch'] = 3
        self.assertTrue(env['full_repaid_frame'](different_version, SOURCE))
        for category in ['group','coverage','identity','pid','partial','credit','replaced','source','pause']:
            with self.subTest(category=category):
                invalid = copy.deepcopy(positive)
                if category == 'group': invalid['group_capture_complete'] = False
                if category == 'coverage': invalid['r07_coverage_complete'] = False
                if category == 'identity': invalid['r07_applied']['owner']['start_abstime'] += 1
                if category == 'pid': invalid['r07_applied']['owner']['pid'] += 1
                if category == 'partial': invalid['r07_applied']['state']['mapped_bytes'] -= 65536
                if category == 'credit': invalid['r07_applied']['permitted_bytes'] = 1
                if category == 'replaced': invalid['r07_applied']['state']['replaced_bytes'] = 65536
                if category == 'source': invalid['r07_applied']['source'] = 'wrong'
                if category == 'pause': invalid['r07_provider_paused'] = False
                self.assertFalse(env['full_repaid_frame'](invalid, SOURCE))

if __name__ == '__main__':
    unittest.main()
