import ast
import importlib.util
from pathlib import Path
import unittest


SCRIPT = Path(__file__).with_name("launch_native.py")
SPEC = importlib.util.spec_from_file_location("launch_native_under_test", SCRIPT)
MODULE = importlib.util.module_from_spec(SPEC)
SPEC.loader.exec_module(MODULE)


class LaunchNativePureTests(unittest.TestCase):
    def test_actual_socket_path_is_short_enough(self):
        path = MODULE.BASE / "combined-control/model/normal-pool/pool.sock"
        self.assertEqual(MODULE.socket_bytes(path), 93)
        self.assertLess(MODULE.socket_bytes(path), MODULE.SUN_PATH_BYTES)

    def test_boundary_counts_utf8_and_nul(self):
        self.assertEqual(MODULE.socket_bytes(Path("a" * 102)), 103)
        self.assertEqual(MODULE.socket_bytes(Path("a" * 103)), 104)

    def test_success_requires_fixed_identity_and_diagnostic_only_receipt(self):
        receipt = {"mode": "combined-control", "passed_controlled_exercise": True,
                   "performance_acceptance": "not_claimed", "source_sha256": "a" * 64,
                   "binary_sha256": "b" * 64}
        self.assertTrue(MODULE.controller_result_ok(0, receipt, "a" * 64, "b" * 64))
        self.assertFalse(MODULE.controller_result_ok(1, receipt, "a" * 64, "b" * 64))
        self.assertFalse(MODULE.controller_result_ok(0, receipt, "c" * 64, "b" * 64))
        receipt["passed_controlled_exercise"] = False
        self.assertFalse(MODULE.controller_result_ok(0, receipt, "a" * 64, "b" * 64))

    def test_import_does_not_launch_child_or_create_attempt(self):
        self.assertFalse((MODULE.BASE / "intent.json").exists())
        tree = ast.parse(SCRIPT.read_text())
        main_guard = [node for node in tree.body if isinstance(node, ast.If)
                      and isinstance(node.test, ast.Compare)]
        self.assertTrue(main_guard)
        self.assertIn("subprocess.Popen", SCRIPT.read_text())

    def test_deadline_is_original_controller_deadline(self):
        controller = (MODULE.BASE / "fault-cases.py").read_text()
        self.assertIn("DEADLINE_SECONDS = 120.0", controller)
        self.assertEqual(MODULE.CONTROLLER_DEADLINE_SECONDS, 120)


if __name__ == "__main__":
    unittest.main()
