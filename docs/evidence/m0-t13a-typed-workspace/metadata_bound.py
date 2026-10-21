"""Conservative profile calculation, not an allocator/physical-memory measurement.
Only frozen canonical generator inputs; malformed-index error peaks are separate.
"""
import hashlib
import json
import math
from pathlib import Path

D = Path(__file__).resolve().parent
R = D.parents[2]
K = 1024
types = json.loads((D / 'metadata-types.json').read_text())
assert types['pointer'] == 8 and types['string'] == 24 and types['value'] == 32
assert types['vec_u8'] == 24 and types['mutex_unit'] == 16
rust = Path('/Users/frankqin/.rustup/toolchains/1.85.1-aarch64-apple-darwin/lib/rustlib/src/rust/library/alloc/src/collections/btree/node.rs')
text = rust.read_text()
assert 'const B: usize = 6;' in text and '2 * B - 1' in text
# Leaf fields + 11 key/value slots + 12 internal edges. Add padding,
# allocation alignment/control, then round upward to a whole KiB.
node_fields = 8 + 2 + 2 + 11 * (24 + 32) + 12 * 8
assert node_fields + 128 <= 1024

def string_bound(text):
    n = len(text.encode())
    return ((n + 15) // 16) * 16 + 32

def value_bound(value):
    if isinstance(value, dict):
        n = len(value)
        nodes = 0 if n == 0 else 1 + (n - 1) // 5
        return nodes * 1024 + sum(string_bound(k) + value_bound(v) for k, v in value.items())
    if isinstance(value, list):
        # Twice the next power-of-two array capacity includes a growth overlap;
        # all frozen arrays are exact-size or generated from fixed iterators.
        capacity = 1 << max(2, (max(1, len(value)) - 1).bit_length())
        return 2 * capacity * 32 + 32 + sum(value_bound(v) for v in value)
    if isinstance(value, str):
        return string_bound(value)
    return 0  # primitive storage is inline in parent slots

report = json.loads((D / 'cold-latest82/report.json').read_text())
manifest = report['manifest']
assert manifest['normal_joint_budget_bytes'] == 310 * 1024**3 // 100
assert len(manifest['stubs']) == 26
assert manifest['decode_workspace']['owned_bytes'] == 64 * 1024**2
index_path = R / 'work/report-lifetime-formal-l1-01/observed/model/layout/snapshots/snapshot-000001/index.json'
index = json.loads(index_path.read_text())
assert len(index['files']) == 29 and len(index['manifest']['padding_payload_bytes']) == 7
for path in (index_path, D, R / 'work/typed-workspace-formal-l1-01/observed/model/layout/snapshots'):
    assert len(str(path).encode()) + 64 <= 512

pages = math.ceil(manifest['padding_payload_total_bytes'] / (1024**2)) + 7
# PressurePage is two machine words. ArcInner<Vec<u8>> contains two
# counters + Vec header; 64B per control includes alignment/slack.
structures = (2 * pages * 32 + (pages + 64) * 64 + 64 * 24
              + 32 * 128 + 16 * K)
manifest_bound = value_bound(manifest)
native_bound = value_bound(manifest['native'])
snapshot_result = dict(generation=128, write_fsync_seconds=0., durable_complete_seconds=0.,
                       files=index['files'], native_restore_bound=report['restore']['native_restore_bound'],
                       complete_record_restore='passed', full_pressure_file_readback='passed',
                       remaining_free_bytes=10**12, cow_backings_returned_after_join=64)
snapshot_bound = value_bound(snapshot_result)
# One schema tree plus two result/event trees; borrowed index has no DOM.
# Add BTree split/new-root temporaries and all bounded paths/control strings.
# Capture one canonical scan-event shape; add the two new workspace fields.
event_path = R / 'work/report-lifetime-formal-l1-01/observed/model/layout/events.jsonl'
with event_path.open() as f:
    for line in f:
        event = json.loads(line)
        if 'scan' in event:
            event_line_sha = hashlib.sha256(line.encode()).hexdigest()
            break
event['scan']['decode_workspace_scanned_bytes'] = 64*1024**2
event['scan']['decode_workspace_borrowed_bytes'] = 64*1024**2
event_upper = value_bound(event)
assert event_upper <= 32*K
# Explicit simultaneous control/temporary envelope, rather than unnamed slack:
# 32 bounded path/argument buffers (<=512 text, <=1024 capacity each);
# one main-thread scan event; 16 bounded status/error strings; owner/thread
# control + Vec/Mutex/JoinHandle headers; and transient split/new-root nodes.
control_items = dict(paths_and_arguments=32*1024, scan_event=32*K,
                     bounded_status_strings=16*256, thread_and_format_control=16*K,
                     btree_insert_transients=16*K)
control = sum(control_items.values())
assert control <= 128*K
# 29 typed FileEntry heads + bounded exact-created name/hash allocations,
# borrowed index header and serializer state fit this extra 16KiB.
entry_control = 29*(64+128+128+64) + 1024
assert entry_control <= 16*K
snapshot_peak = manifest_bound + 2 * snapshot_bound + 16 * K + control
# Canonical raw plus worst growth-overlap scratch based on 2x Vec growth.
# Kept identifiers: <=128 bytes each; 128 covers every fixed retained name.
# Native comparison holds at most two small trees, not two full manifests.
cold_peak = 256 * K + 3 * 256 * K + 128 * (128 + 32) + 16 * K + 2 * native_bound + control
summary = dict(generation=128, durable_complete_seconds=0., write_fsync_seconds=0.,
               file_count=29, file_bytes=3378784402, record_restore_passed=True,
               pressure_readback_passed=True)
class_names = [
    'business_transaction', 'business_solver', 'periodic_obligation', 'checkpoint',
    'restore', 'transport', 'environment', 'spatial',
    'L1_native_record_index_page', 'L1_stub_record_index_page',
]
# Use ten maximum-width class names rather than rely on the subset active in L1.
quantile = dict(nearest_rank=10**12, lower_exclusive_ns=10**12, upper_inclusive_ns=10**12)
histogram = dict(samples=10**12, overflow_40ms_samples=10**12, maximum_ns=10**12,
                 p95=quantile, p99=quantile, gate='not_passed_insufficient_samples')
metrics = dict(schema='mycity-compute-latencies-v1', raw_encoding='9 bytes: class u8, duration_ns u64 little endian',
               raw_bytes=10**12, diagnostic_owned_bytes=10**12,
               quantiles='nearest rank with conservative 1us bounds; raw file supports exact audit',
               classes={('x'*64)+name: histogram for name in class_names})
final_template = dict(
    schema='mycity-layout-probe-v2', owner_session=True, publication_end_seconds=28800.,
    snapshot_summaries_only=True,
    diagnostics=dict(allocated_owned_bytes=10**12, transient_reservation_bytes=256*K,
                     charged_bound_bytes=10**12, cap_bytes=10**12,
                     full_snapshot_file_details='events.jsonl and retained snapshot indices'),
    source_sha256='a'*64, requested_wall_seconds=28800, work_wall_seconds=28800.,
    initialization_seconds=10., snapshot_interval_seconds=300, scans=28800,
    expected_published_scans=28800, all_published_scans_completed=True,
    manifest=manifest, snapshots=[summary]*128,
    maximum_scan_ready_age_seconds=10., maximum_snapshot_start_lateness_seconds=10.,
    compute=metrics, diagnostic_outcome={'Ok': None}, full_layout_assembled=True,
    steady_gate='not_accepted_requires_three_runs_and_group_system_measurements',
    eight_hour_gate='not_run', memory_16gb_gate='not_run', full_M0='not_accepted')
# Covers the report plus prior manifest/metrics trees and serialization control.
final_peak = value_bound(final_template) + manifest_bound + value_bound(metrics) + control
io = 2 * 1024**2
stages = {name: io + structures + peak for name, peak in
          [('snapshot', snapshot_peak), ('cold_restore', cold_peak), ('final_report', final_peak)]}
assert max(stages.values()) <= 4 * 1024**2, stages
result = dict(scope='frozen canonical v1/v2 profile arithmetic; NOT physical memory or malformed-input proof',
              source_sha256=json.loads((D/'source.json').read_text())['source_sha256'],
              io_backings_bytes=io, structure_upper_bytes=structures,
              maximum_pressure_index_capacity=pages, manifest_tree_upper_bytes=manifest_bound,
              snapshot_result_tree_upper_bytes=snapshot_bound, native_tree_upper_bytes=native_bound,
              scan_event_tree_upper_bytes=event_upper, control_items=control_items,
              canonical_string_construction='to_owned/format of bounded fixed identifiers; no previously grown/shrunk String credited by final length',
              array_construction='fixed arrays and fixed iterator collect; 2x power-of-two capacity includes a growth overlap',
              stage_upper_bytes=stages, cap_bytes=4*1024**2,
              margin_bytes=4*1024**2-max(stages.values()),
              btree_source_sha256=hashlib.sha256(rust.read_bytes()).hexdigest(),
              inputs_sha256={str(p.relative_to(R)):hashlib.sha256(p.read_bytes()).hexdigest() for p in
                             [D/'cold-latest82/report.json', index_path, D/'metadata-types.json',
                              D/'metadata-types.rs', D/'metadata-rustc.txt',
                              R/'work/candidate-binaries/typed-workspace-metadata/metadata-types']},
              canonical_scan_event_line_sha256=event_line_sha,
              node_field_sum_bytes=node_fields, charged_node_bytes=1024,
              physical_memory_gate='not_accepted', error_path_budget='not_proven',
              runtime_guard='external preflight only; not a general CLI allocation limit',
              native_thread_stacks='excluded from Rust metadata; physical gate still required',
              diagnostics='histograms/events/summary backing remain in original separate charge; no credit subtracted here')
(D/'metadata-bound.json').write_text(json.dumps(result,indent=2)+'\n')
print(json.dumps(result,indent=2))
