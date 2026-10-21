import unittest
from clock_exchange import bounds,SCHEMA
class BoundsTests(unittest.TestCase):
 def reply(self,**changes):
  r=dict(schema=SCHEMA,pid=12,source_sha256='source',nonce='nonce',elapsed_ns=40);r.update(changes);return r
 def test_actual_interval_and_spawn_intersection(self):
  self.assertEqual(bounds(self.reply(),12,'source','nonce',100,110,3),(60,70))
  self.assertEqual(bounds(self.reply(),12,'source','nonce',100,110,65),(65,70))
 def test_wrong_identity_cannot_authorize_mapping(self):
  for changes in (dict(pid=13),dict(source_sha256='wrong'),dict(nonce='old'),dict(schema='old')):
   with self.assertRaises(RuntimeError):bounds(self.reply(**changes),12,'source','nonce',100,110,3)
 def test_invalid_timestamp_and_empty_intersection_fail(self):
  for changes in (dict(elapsed_ns=True),dict(elapsed_ns=-1),dict(elapsed_ns=120),dict(elapsed_ns=1.0)):
   with self.assertRaises(RuntimeError):bounds(self.reply(**changes),12,'source','nonce',100,110,3)
  with self.assertRaises(RuntimeError):bounds(self.reply(),12,'source','nonce',100,110,75)
  with self.assertRaises(RuntimeError):bounds(self.reply(),12,'source','nonce',111,110,3)
if __name__=='__main__':unittest.main()
