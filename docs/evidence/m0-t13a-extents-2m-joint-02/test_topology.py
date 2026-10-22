import unittest

from topology import assert_pinned_identity, assert_repaid_identity, r07_extents, MAX_EXTENT_BYTES

Q = 65_536
M = MAX_EXTENT_BYTES
C = M // Q


def extent(start, quanta, address, generation):
    return {
        "kind": "r07_immutable_map",
        "ordinal": 0,
        "allocation_generation": generation,
        "start": address,
        "len": quanta * Q,
        "capacity": quanta * Q,
        "logical_start": start * Q,
        "logical_len": quanta * Q,
    }


class TopologyTests(unittest.TestCase):
    def test_contiguous_intervals_and_total_mapped_bytes(self):
        ranges = [extent(0, C, 0x100000, 3), extent(C, 4, 0x300000, 8)]
        self.assertEqual(len(r07_extents({"ranges": ranges}, (C+4) * Q)), 2)

    def test_rejects_logical_overlap_gap_and_mapped_byte_mismatch(self):
        valid = [extent(0, C, 0x100000, 3), extent(C, 4, 0x300000, 8)]
        overlap = [dict(valid[0]), dict(valid[1], logical_start=(C-1) * Q)]
        gap = [dict(valid[0]), dict(valid[1], logical_start=(C+1) * Q)]
        for candidate, total in ((overlap, (C+4) * Q), (gap, (C+4) * Q), (valid, (C+3) * Q)):
            with self.subTest(candidate=candidate, total=total), self.assertRaises(AssertionError):
                r07_extents({"ranges": candidate}, total)

    def test_pin_requires_every_extent_address_and_generation_stable(self):
        before = [extent(0, C, 0x100000, 3), extent(C, 4, 0x300000, 8)]
        assert_pinned_identity(before, [dict(item) for item in before])
        changed = [dict(before[0]), dict(before[1], start=0x400000)]
        with self.assertRaises(AssertionError):
            assert_pinned_identity(before, changed)
        changed = [dict(before[0]), dict(before[1], allocation_generation=9)]
        with self.assertRaises(AssertionError):
            assert_pinned_identity(before, changed)

    def test_repayment_preserves_full_extent_and_allows_one_new_generation_tail(self):
        before = [extent(0, C, 0x100000, 3), extent(C, 4, 0x300000, 8)]
        after = [extent(0, C, 0x100000, 3), extent(C, C, 0x400000, 10)]
        assert_repaid_identity(before, after, 2 * C * Q)

    def test_repayment_can_preserve_pinned_short_tail_and_append_new_extents(self):
        before = [extent(0, C, 0x100000, 3), extent(C, 4, 0x300000, 8)]
        after = [
            extent(0, C, 0x100000, 3),
            extent(C, 4, 0x300000, 8),
            extent(C+4, C-4, 0x500000, 9),
        ]
        assert_repaid_identity(before, after, 2 * C * Q)

    def test_repayment_rejects_replaced_full_extent_and_old_generation_reuse(self):
        before = [extent(0, C, 0x100000, 3), extent(C, 4, 0x300000, 8)]
        replaced_full = [extent(0, C, 0x600000, 10), extent(C, C, 0x400000, 11)]
        with self.assertRaises(AssertionError):
            assert_repaid_identity(before, replaced_full, 2 * C * Q)
        bad_tail = [extent(0, C, 0x100000, 3), extent(C, C, 0x400000, 8)]
        with self.assertRaises(AssertionError):
            assert_repaid_identity(before, bad_tail, 2 * C * Q)


if __name__ == "__main__":
    unittest.main()
