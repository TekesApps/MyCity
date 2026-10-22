import unittest,zipfile,re
from pathlib import Path
from topology import MAX_EXTENT_BYTES,r07_extents
from test_topology import extent,Q
class ExtentContractTests(unittest.TestCase):
    def test_frozen_rust_and_observer_ceiling_match(self):
        with zipfile.ZipFile(Path(__file__).parent/"frozen-source.zip") as z:
            code=z.read("crates/city-headless/src/layout_load/r07.rs").decode()
        q=int(re.search(r"MAX_EXTENT_QUANTA: usize = (\d+)",code).group(1))
        self.assertEqual(q*Q,MAX_EXTENT_BYTES)
    def test_accept_ceiling_and_reject_one_extra_quantum(self):
        r07_extents({"ranges":[extent(0,MAX_EXTENT_BYTES//Q,0x100000,1)]},MAX_EXTENT_BYTES)
        with self.assertRaises(AssertionError):
            r07_extents({"ranges":[extent(0,MAX_EXTENT_BYTES//Q+1,0x100000,1)]},MAX_EXTENT_BYTES+Q)
