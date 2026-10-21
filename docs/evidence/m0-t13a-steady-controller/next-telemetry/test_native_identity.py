import unittest
from unittest.mock import patch
from types import SimpleNamespace
import observe_native_candidate as o
class Identity(unittest.TestCase):
 def bound(self,readings,relation=(10,None,'2','model'),expected=None,cached=None):
  with patch.object(o,'usage',side_effect=readings),patch.object(o,'native',SimpleNamespace(relation=lambda pid:relation),create=True):
   return o.bound_usage(20,(10,None,'2','model'),expected,cached)
 def test_stable_birth_and_relation_accept(self):
  result,issue,proof=self.bound([{'start_abstime':100},{'start_abstime':100}],expected=100)
  self.assertEqual(result['start_abstime'],100);self.assertIsNone(issue);self.assertLessEqual(proof['capture_begin_monotonic_ns'],proof['capture_end_monotonic_ns'])
 def test_pid_reuse_during_relation_capture_is_unknown(self):
  result,issue,_=self.bound([{'start_abstime':100},{'start_abstime':200}],expected=100)
  self.assertIsNone(result);self.assertEqual(issue['reason'],'birth_changed_during_relation_capture')
 def test_previous_frame_identity_does_not_transfer(self):
  result,issue,_=self.bound([{'start_abstime':200},{'start_abstime':200}],expected=100)
  self.assertIsNone(result);self.assertEqual(issue['reason'],'expected_birth_changed')
 def test_reparented_or_reused_new_member_is_unknown(self):
  result,issue,_=self.bound([{'start_abstime':200},{'start_abstime':200}],relation=(999,None,'2','model'))
  self.assertIsNone(result);self.assertEqual(issue['reason'],'enumerated_relation_changed')
 def test_temporary_missing_never_becomes_zero(self):
  result,issue,_=self.bound([{'start_abstime':100},None],expected=100)
  self.assertIsNone(result);self.assertEqual(issue['reason'],'identity_or_relation_unavailable')
 def test_cached_identity_keeps_its_capture_bracket(self):
  result,issue,proof=self.bound([{'start_abstime':100}],expected=100,cached=(123,{'start_abstime':100}))
  self.assertIsNone(issue);self.assertEqual(proof['capture_begin_monotonic_ns'],123)
if __name__=='__main__':unittest.main()
