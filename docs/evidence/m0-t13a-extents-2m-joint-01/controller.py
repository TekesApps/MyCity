"""Fail-closed observation and child-lifecycle helpers for the retry harness."""
import subprocess
import time


def wait_for_context(fetch, expected_phase, child, timeout=90.0, interval=0.01,
                     monotonic=time.monotonic, sleep=time.sleep):
    """Wait through intentional ack gaps; never treat missing context as success."""
    deadline = monotonic() + timeout
    while monotonic() < deadline:
        if child.poll() is not None:
            raise RuntimeError("existing wrapper exited before a valid observation context")
        context = fetch()
        if context is not None and context.get("phase") == expected_phase:
            state = context.get("ack", {}).get("state")
            topology = context.get("topology", {})
            header = topology.get("header", {})
            epoch = state.get("epoch") if isinstance(state, dict) else None
            if (isinstance(epoch, int) and epoch == header.get("epoch")
                    and topology.get("sha256")):
                return context
        sleep(interval)
    raise TimeoutError(f"no valid {expected_phase!r} observation context before timeout")


def assert_stable_context(before, after, expected_phase):
    """Require valid same-phase, same-epoch, same-list endpoints around vmmap."""
    assert before is not None and after is not None, "missing context is not a valid sample"
    assert before["phase"] == after["phase"] == expected_phase
    before_epoch = before["ack"]["state"]["epoch"]
    after_epoch = after["ack"]["state"]["epoch"]
    assert before_epoch == after_epoch
    assert before_epoch == before["topology"]["header"]["epoch"]
    assert after_epoch == after["topology"]["header"]["epoch"]
    assert before["topology"]["sha256"] == after["topology"]["sha256"]
    return True


def topology_for_ack(ack, path, read_header, load_topology):
    """Check list epoch before validating its mapped length against the ack."""
    if not ack or not isinstance(ack.get("state"), dict):
        return None
    state = ack["state"]
    if not isinstance(state.get("mapped_bytes"), int):
        return None
    header = read_header(path)
    assert header.get("record") == "begin"
    if state.get("epoch") != header.get("epoch"):
        return None
    # Same-epoch mismatches are data integrity errors and must remain fatal.
    return load_topology(path, state["mapped_bytes"])


def bind_pinned_baseline(admission, inflight_context):
    """Bind extents only if the admission event carries matching R07 State."""
    state = admission.get("r07_state", admission.get("r07"))
    if isinstance(state, dict) and isinstance(state.get("state"), dict):
        state = state["state"]
    if not isinstance(state, dict):
        return None, {
            "claim": "not_made",
            "reason": "snapshot_admitted payload has no R07 State to bind",
        }
    epoch = state.get("epoch")
    mapped_bytes = state.get("mapped_bytes")
    observed = inflight_context["ack"]["state"]
    list_epoch = inflight_context["topology"]["header"]["epoch"]
    if not isinstance(epoch, int) or not isinstance(mapped_bytes, int):
        return None, {
            "claim": "not_made",
            "reason": "snapshot_admitted R07 State lacks epoch or mapped_bytes",
        }
    assert epoch == observed.get("epoch") == list_epoch
    assert mapped_bytes == observed.get("mapped_bytes") == inflight_context["topology"]["r07_mapped_bytes"]
    return inflight_context["topology"]["r07_extents"], {
        "claim": "bound_to_snapshot_admission_r07_state",
        "epoch": epoch,
        "mapped_bytes": mapped_bytes,
    }


def wait_for_existing_child(child, on_wait, interval=30.0):
    """Wait until the original child exits; never restart or terminate it."""
    while True:
        code = child.poll()
        if code is not None:
            return child.wait()
        try:
            return child.wait(timeout=interval)
        except subprocess.TimeoutExpired:
            try:
                on_wait(child.poll())
            except BaseException:
                # A receipt-write problem must not abandon the already-running
                # child after the controller has launched it.
                continue


def initialize_workspace(work, intent, minimum_free_bytes=10 * 1024**3):
    """Create once; reject old state and check the actual work volume."""
    import os
    volume = os.statvfs(work.parent)
    available = volume.f_bavail * volume.f_frsize
    if available <= minimum_free_bytes:
        raise RuntimeError("insufficient work-volume space")
    work.mkdir()
    root = work / "observed"
    (work / "intent.json").write_bytes(intent)
    return root
