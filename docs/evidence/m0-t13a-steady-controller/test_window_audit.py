"""Counterexamples for conservative time/range selection, no model workload."""
import importlib.util,tempfile,unittest
from pathlib import Path
s=importlib.util.spec_from_file_location('window_audit',Path(__file__).with_name('window_audit.py'));m=importlib.util.module_from_spec(s);s.loader.exec_module(m)
class Window(unittest.TestCase):
 def test_whole_batch_and_all_mapping_uncertainty_required(self):
  self.assertEqual(m.disposition((100,110,180,190),100,200),'inside')
  self.assertEqual(m.disposition((99,110,180,190),100,200),'cross_boundary_or_uncertain')
  self.assertEqual(m.disposition((100,110,190,201),100,200),'cross_boundary_or_uncertain')
  self.assertEqual(m.disposition(None,100,200),'unknown_clock_or_incomplete')
  self.assertEqual(m.disposition((10,11,20,21),100,200),'before')
  self.assertEqual(m.disposition((200,201,210,211),100,200),'after')
 def test_instance_mismatch_and_inconsistent_calibration_refused(self):
  a=dict(pid=1,instance_unix_ns=1,elapsed_ns=10);b=dict(a,instance_unix_ns=2,elapsed_ns=20)
  with self.assertRaises(AssertionError):m.clock_bounds(a,b,{(1,1):(90,100)})
  with self.assertRaises(AssertionError):m.intersect((0,1),(2,3))
 def test_range_gap_and_duplicate_refused(self):
  with tempfile.TemporaryDirectory() as d:
   p=Path(d)/'raw';p.write_bytes(bytes([0])+int(12).to_bytes(8,'little'))
   a=object.__new__(m.Audit);a.files={};a.raw_counts=m.collections.defaultdict(m.collections.Counter);a.raw_max=m.collections.defaultdict(lambda:m.collections.defaultdict(int));a.hist=m.collections.defaultdict(m.collections.Counter);a.batches=m.collections.Counter()
   tag=('sealed','sim','execution','inside','confirmed_applied','economy/world/pe0')
   with self.assertRaises(AssertionError):a.segment(p,9,9,tag)
   a.segment(p,0,9,tag)
   with self.assertRaises(AssertionError):a.segment(p,0,9,tag)
 def test_unbound_raw_is_in_universe_and_never_eligible(self):
  with tempfile.TemporaryDirectory() as d:
   root=Path(d);b=root/'business';b.mkdir();raw=b/'orphan/compute-durations.bin';raw.parent.mkdir();raw.write_bytes(bytes([0])+int(12).to_bytes(8,'little'))
   a=object.__new__(m.Audit);a.root=root;a.b=b;a.files={};a.raw_counts=m.collections.defaultdict(m.collections.Counter);a.raw_max=m.collections.defaultdict(lambda:m.collections.defaultdict(int));a.hist=m.collections.defaultdict(m.collections.Counter);a.batches=m.collections.Counter();a.unmapped=[]
   a.raw_universe();self.assertEqual(len(a.unmapped),1);self.assertEqual(a.files[str(raw)],9)
   self.assertTrue(all(k[0]=='unbound' for k in a.hist))
 def test_different_cases_and_air_specs_never_pool(self):
  with tempfile.TemporaryDirectory() as d:
   a=object.__new__(m.Audit);a.files={};a.raw_counts=m.collections.defaultdict(m.collections.Counter);a.raw_max=m.collections.defaultdict(lambda:m.collections.defaultdict(int));a.hist=m.collections.defaultdict(m.collections.Counter);a.batches=m.collections.Counter()
   for case,spec in [('debt',[[256,256],[256,256]]),('refund',[[128,256],[256,256]])]:
    p=Path(d)/case;p.write_bytes(bytes([6])+int(12).to_bytes(8,'little'))
    a.segment(p,0,9,('sealed','sim','execution','inside','confirmed_applied',case),spec)
   self.assertEqual(len(a.hist),2)
   self.assertEqual({k[-1] for k in a.hist},{'page_limit_256','unproven_air_spec'})
if __name__=='__main__':unittest.main()
