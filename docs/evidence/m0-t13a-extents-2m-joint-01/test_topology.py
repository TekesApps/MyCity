import unittest

from topology import assert_pinned_identity, assert_repaid_identity, r07_extents

Q = 65_536
M = 16 * Q


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
        ranges = [extent(0, 16, 0x100000, 3), extent(16, 4, 0x300000, 8)]
        self.assertEqual(len(r07_extents({"ranges": ranges}, 20 * Q)), 2)

    def test_rejects_logical_overlap_gap_and_mapped_byte_mismatch(self):
        valid = [extent(0, 16, 0x100000, 3), extent(16, 4, 0x300000, 8)]
        overlap = [dict(valid[0]), dict(valid[1], logical_start=15 * Q)]
        gap = [dict(valid[0]), dict(valid[1], logical_start=17 * Q)]
        for candidate, total in ((overlap, 20 * Q), (gap, 20 * Q), (valid, 19 * Q)):
            with self.subTest(candidate=candidate, total=total), self.assertRaises(AssertionError):
                r07_extents({"ranges": candidate}, total)

    def test_pin_requires_every_extent_address_and_generation_stable(self):
        before = [extent(0, 16, 0x100000, 3), extent(16, 4, 0x300000, 8)]
        assert_pinned_identity(before, [dict(item) for item in before])
        changed = [dict(before[0]), dict(before[1], start=0x400000)]
        with self.assertRaises(AssertionError):
            assert_pinned_identity(before, changed)
        changed = [dict(before[0]), dict(before[1], allocation_generation=9)]
        with self.assertRaises(AssertionError):
            assert_pinned_identity(before, changed)

    def test_repayment_preserves_full_extent_and_allows_one_new_generation_tail(self):
        before = [extent(0, 16, 0x100000, 3), extent(16, 4, 0x300000, 8)]
        after = [extent(0, 16, 0x100000, 3), extent(16, 16, 0x400000, 10)]
        assert_repaid_identity(before, after, 32 * Q)

    def test_repayment_can_preserve_pinned_short_tail_and_append_new_extents(self):
        before = [extent(0, 16, 0x100000, 3), extent(16, 4, 0x300000, 8)]
        after = [
            extent(0, 16, 0x100000, 3),
            extent(16, 4, 0x300000, 8),
            extent(20, 12, 0x500000, 9),
        ]
        assert_repaid_identity(before, after, 32 * Q)

    def test_repayment_rejects_replaced_full_extent_and_old_generation_reuse(self):
        before = [extent(0, 16, 0x100000, 3), extent(16, 4, 0x300000, 8)]
        replaced_full = [extent(0, 16, 0x600000, 10), extent(16, 16, 0x400000, 11)]
        with self.assertRaises(AssertionError):
            assert_repaid_identity(before, replaced_full, 32 * Q)
        bad_tail = [extent(0, 16, 0x100000, 3), extent(16, 16, 0x400000, 8)]
        with self.assertRaises(AssertionError):
            assert_repaid_identity(before, bad_tail, 32 * Q)


if __name__ == "__main__":
    unittest.main()
