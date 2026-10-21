import struct
import tempfile
from pathlib import Path
import unittest
from decode_trace import decode

class TraceTests(unittest.TestCase):
    def check_bytes(self, raw, valid):
        with tempfile.TemporaryDirectory(prefix='mycity-trace-') as d:
            p = Path(d)/'trace.bin'
            p.write_bytes(raw)
            if valid:
                r = decode(p)
                self.assertTrue(r['final_decode_repayment_observed'])
                self.assertEqual(r['records'], 1)
            else:
                with self.assertRaises(ValueError): decode(p)

    def test_fixed_record_and_corruption(self):
        good = [0x4d59435452414301,0,2,9,0,0,100,100,200,0,120,8,0,123,0,0]
        self.check_bytes(struct.pack('<16Q', *good), True)
        for index, value in [(0,0),(1,1),(3,99),(12,9),(14,1)]:
            bad=good.copy();bad[index]=value
            self.check_bytes(struct.pack('<16Q', *bad), False)
        for bad in [b'', b'x', struct.pack('<16Q', *good)[:-1], b'x'*(128*256+1)]:
            self.check_bytes(bad,False)

if __name__ == '__main__': unittest.main()
