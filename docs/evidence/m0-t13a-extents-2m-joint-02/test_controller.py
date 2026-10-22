import subprocess
import unittest

from controller import (
    assert_stable_context,
    bind_pinned_baseline,
    topology_for_ack,
    wait_for_context,
    wait_for_existing_child,
)
from topology import assert_pinned_identity


def context(epoch=4, phase="inflight", digest="list-sha", list_epoch=None):
    return {
        "phase": phase,
        "ack": {"state": {"epoch": epoch}},
        "topology": {"sha256": digest, "header": {"epoch": epoch if list_epoch is None else list_epoch}},
    }


class FakeChild:
    def __init__(self, exit_after=None):
        self.exit_after = exit_after
        self.polls = 0
        self.wait_calls = 0
        self.timeouts = 0

    def poll(self):
        self.polls += 1
        return 1 if self.exit_after is not None and self.polls >= self.exit_after else None

    def wait(self, timeout=None):
        self.wait_calls += 1
        if self.timeouts:
            self.timeouts -= 1
            raise subprocess.TimeoutExpired("existing-child", timeout)
        return 0


class ControllerTests(unittest.TestCase):
    def test_ack_gap_waits_until_valid_context_and_does_not_count_gap(self):
        sequence = iter((None, None, context(list_epoch=3), context()))
        now = [0.0]

        def sleep(seconds):
            now[0] += seconds

        child = FakeChild()
        got = wait_for_context(
            lambda: next(sequence), "inflight", child,
            timeout=1.0, interval=0.1,
            monotonic=lambda: now[0], sleep=sleep,
        )
        self.assertEqual(got, context())
        self.assertAlmostEqual(now[0], 0.3)

    def test_ack_absence_until_child_exit_is_failure(self):
        child = FakeChild(exit_after=3)
        with self.assertRaisesRegex(RuntimeError, "existing wrapper exited"):
            wait_for_context(lambda: None, "inflight", child, timeout=1.0)

    def test_context_endpoints_must_keep_epoch_phase_and_list(self):
        self.assertTrue(assert_stable_context(context(), context(), "inflight"))
        for changed in (
            context(epoch=5),
            context(phase="idle"),
            context(digest="different-list"),
        ):
            with self.subTest(changed=changed), self.assertRaises(AssertionError):
                assert_stable_context(context(), changed, "inflight")
        with self.assertRaises(AssertionError):
            assert_stable_context(context(), context(list_epoch=5), "inflight")

    def test_exception_path_waits_for_existing_child_after_timeout(self):
        child = FakeChild()
        child.timeouts = 1
        waits = []
        self.assertEqual(wait_for_existing_child(child, waits.append, interval=0.1), 0)
        self.assertEqual(waits, [None])
        self.assertEqual(child.wait_calls, 2)

    def test_stale_list_epoch_waits_before_mapped_length_validation(self):
        calls = []

        def loader(path, mapped):
            calls.append((path, mapped))
            raise AssertionError("stale range length should not be compared to newer ack")

        result = topology_for_ack(
            {"state": {"epoch": 8, "mapped_bytes": 1024}},
            "old-list.jsonl",
            lambda _path: {"record": "begin", "epoch": 7},
            loader,
        )
        self.assertIsNone(result)
        self.assertEqual(calls, [])

    def test_same_epoch_mapped_length_mismatch_remains_fatal(self):
        calls = []

        def loader(path, mapped):
            calls.append((path, mapped))
            raise AssertionError("same-epoch logical range sum mismatch")

        with self.assertRaisesRegex(AssertionError, "same-epoch"):
            topology_for_ack(
                {"state": {"epoch": 8, "mapped_bytes": 2048}},
                "same-epoch-list.jsonl",
                lambda _path: {"record": "begin", "epoch": 8},
                loader,
            )
        self.assertEqual(calls, [("same-epoch-list.jsonl", 2048)])

    def test_legal_pre_pin_tail_change_is_not_compared_to_idle_identity(self):
        idle_tail = {"logical_start": 16, "logical_len": 4, "capacity": 4, "start": 0x3000, "allocation_generation": 8}
        inflight_tail = {"logical_start": 16, "logical_len": 8, "capacity": 8, "start": 0x5000, "allocation_generation": 9}
        inflight = context(epoch=12)
        inflight["ack"]["state"]["mapped_bytes"] = 24
        inflight["topology"]["r07_mapped_bytes"] = 24
        inflight["topology"]["r07_extents"] = [inflight_tail]
        baseline, claim = bind_pinned_baseline(
            {"generation": 1, "available_bytes": 10}, inflight
        )
        self.assertEqual(idle_tail["allocation_generation"], 8)
        self.assertEqual(inflight_tail["allocation_generation"], 9)
        self.assertIsNone(baseline)
        self.assertEqual(claim["claim"], "not_made")
        with self.assertRaises(AssertionError):
            assert_pinned_identity([idle_tail], [inflight_tail])
        if baseline is not None:
            assert_pinned_identity(baseline, [inflight_tail])

    def test_pinned_baseline_binds_only_matching_admission_r07_state(self):
        observed = context(epoch=12)
        observed["ack"]["state"]["mapped_bytes"] = 24
        observed["topology"]["r07_mapped_bytes"] = 24
        observed["topology"]["r07_extents"] = [{"logical_start": 0, "logical_len": 24, "start": 0x7000, "allocation_generation": 10}]
        baseline, claim = bind_pinned_baseline(
            {"generation": 1, "r07_state": {"epoch": 12, "mapped_bytes": 24}}, observed
        )
        self.assertEqual(baseline, observed["topology"]["r07_extents"])
        self.assertEqual(claim["claim"], "bound_to_snapshot_admission_r07_state")
        with self.assertRaises(AssertionError):
            bind_pinned_baseline(
                {"generation": 1, "r07_state": {"epoch": 11, "mapped_bytes": 24}}, observed
            )
        with self.assertRaises(AssertionError):
            bind_pinned_baseline(
                {"generation": 1, "r07_state": {"epoch": 12, "mapped_bytes": 16}}, observed
            )


if __name__ == "__main__":
    unittest.main()
