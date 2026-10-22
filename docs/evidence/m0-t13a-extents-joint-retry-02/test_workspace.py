import tempfile
import unittest
from pathlib import Path
from types import SimpleNamespace
from unittest.mock import patch
from controller import initialize_workspace

class WorkspaceTests(unittest.TestCase):
    def test_create_once_preserves_old_attempt(self):
        with tempfile.TemporaryDirectory() as tmp:
            work=Path(tmp)/"attempt"
            initialize_workspace(work,b"original",minimum_free_bytes=0)
            with self.assertRaises(FileExistsError):
                initialize_workspace(work,b"replacement",minimum_free_bytes=0)
            self.assertEqual((work/"intent.json").read_bytes(),b"original")
            self.assertFalse((work/"observed").exists())
            (work/"observed").mkdir()  # actual native observer owns exclusive root creation
    def test_capacity_checks_actual_parent_and_creates_nothing_on_failure(self):
        with tempfile.TemporaryDirectory() as tmp:
            work=Path(tmp)/"attempt"
            with patch("os.statvfs",return_value=SimpleNamespace(f_bavail=0,f_frsize=4096)) as measure:
                with self.assertRaises(RuntimeError):initialize_workspace(work,b"x")
                measure.assert_called_once_with(work.parent)
            self.assertFalse(work.exists())
    def test_runner_uses_one_initializer_and_consistent_second_ack(self):
        import ast
        tree=ast.parse((Path(__file__).parent/"run.py").read_text())
        calls=[n for n in ast.walk(tree) if isinstance(n,ast.Call)]
        self.assertEqual(sum(isinstance(n.func,ast.Name) and n.func.id=="initialize_workspace" for n in calls),1)
        self.assertFalse(any(isinstance(n.func,ast.Attribute) and isinstance(n.func.value,ast.Name) and n.func.value.id=="W" and n.func.attr=="mkdir" for n in calls))
