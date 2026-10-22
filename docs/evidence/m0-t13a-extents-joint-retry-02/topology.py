"""Static checks for physical R07 extent topology; no process or pressure I/O."""

MAX_EXTENT_BYTES = 16 * 65_536
LOGICAL_QUANTUM_BYTES = 65_536
R07_KIND = "r07_immutable_map"


def r07_extents(topology, expected_mapped_bytes):
    """Validate real-map ranges against their contiguous logical byte intervals."""
    ranges = [item for item in topology["ranges"] if item.get("kind") == R07_KIND]
    assert ranges, "missing R07 extents"
    ranges.sort(key=lambda item: item.get("logical_start", -1))
    cursor = 0
    physical = []
    for item in ranges:
        start = item.get("start")
        length = item.get("len")
        capacity = item.get("capacity")
        logical_start = item.get("logical_start")
        logical_len = item.get("logical_len")
        generation = item.get("allocation_generation")
        assert isinstance(start, int) and start > 0
        assert isinstance(length, int) and length > 0
        assert capacity == length == logical_len
        assert isinstance(logical_start, int) and logical_start == cursor
        assert logical_len % LOGICAL_QUANTUM_BYTES == 0
        assert logical_len <= MAX_EXTENT_BYTES
        assert isinstance(generation, int) and generation > 0
        assert start + length > start
        cursor += logical_len
        physical.append((start, start + length))
    physical.sort()
    for left, right in zip(physical, physical[1:]):
        assert left[1] <= right[0], "physical mmap ranges overlap"
    assert cursor == expected_mapped_bytes, (cursor, expected_mapped_bytes)
    return ranges


def identity(extent):
    return (
        extent["logical_start"],
        extent["logical_len"],
        extent["start"],
        extent["capacity"],
        extent["allocation_generation"],
    )


def assert_pinned_identity(before, after):
    """A snapshot-pinned interval must retain every extent address and generation."""
    assert [identity(item) for item in before] == [identity(item) for item in after]


def assert_repaid_identity(before, after, mapped_bytes):
    """Require full-map identity, allowing only legal old-tail growth/replacement."""
    after = r07_extents({"ranges": after}, mapped_bytes)
    old_mapped = sum(item["logical_len"] for item in before)
    old_max_generation = max(item["allocation_generation"] for item in before)
    after_by_start = {item["logical_start"]: item for item in after}
    old_starts = {item["logical_start"] for item in before}
    replaced_tail_count = 0

    for old in before:
        current = after_by_start.get(old["logical_start"])
        assert current is not None, "repaid topology dropped an old logical extent"
        exact = identity(old) == identity(current)
        if old["logical_len"] == MAX_EXTENT_BYTES or (
            old["logical_start"] + old["logical_len"] < old_mapped
        ):
            assert exact, "full or non-tail extent identity changed"
            continue
        if not exact:
            replaced_tail_count += 1
            assert old["logical_start"] + old["logical_len"] == old_mapped
            assert current["logical_len"] > old["logical_len"]
            assert current["allocation_generation"] > old_max_generation
    assert replaced_tail_count <= 1, "more than one old short tail was replaced"

    for item in after:
        if item["logical_start"] not in old_starts:
            assert item["logical_start"] >= old_mapped
            assert item["allocation_generation"] > old_max_generation
    return after
