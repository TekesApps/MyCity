"""A55 design arithmetic only. Does not execute draft Rust or certify a runtime."""
from decimal import Decimal as D
from pathlib import Path
import json

root = Path(__file__).resolve().parents[3]
doc = (root / 'docs/process-worker-and-render-architecture.md').read_text()
old = json.loads((root / 'docs/inputs/product-blueprint/reference-values.json').read_text())
assert sum(D(str(v)) for v in old['app_gib'].values()) == D('3.10')
runtime = D('0.45') * 1024
runtime_reserved = 96 + 64 + 8 + 128
assert runtime - runtime_reserved == D('164.8')
assert 16 + 2 * 32 + 16 == 96
assert 2 * 32 == 64
assert 2 * (8 + 16 + 8) + 32 == 96
assert 192 + 32 == 224
assert 256 - 224 == 32
frame = 3840 * 2160 * 4
assert frame == 33177600
assert D(frame) / (1024 ** 2) == D('31.640625')
assert D(frame * 3) / (1024 ** 2) == D('94.921875')
assert frame * 60 == 1990656000
for field in ['writer incarnation', 'job_generation', 'occurrence', 'phase',
              '规则', 'device lost', 'CPU回退', 'unknown', '应急', 'generation']:
    assert field in doc, field
for gate in ['P-WORKER', 'P-SUPERVISOR', 'P-RENDER-PROC', 'T05-B', 'T13联合']:
    assert gate in doc, gate
report = {
    'release': 'v0.5.20', 'date': '2026-10-20', 'evidence': 'static_design_only',
    'published_model_baseline': 'v0.5.19',
    'draft_t05b': 'uncommitted_not_accepted',
    'app_total_gib': str(D('3.10')), 'runtime_domain_mib': str(runtime),
    'runtime_reserved_mib': runtime_reserved,
    'runtime_remaining_for_existing_uses_mib': str(runtime - runtime_reserved),
    'worker_count_candidate': 2, 'tasks_per_worker_candidate': 1,
    'normal_worker_and_gpu_compute_mib': 96,
    'preview_one_worker_gpu_compute_disabled_mib': 224,
    'rgba8_4k_frame_bytes': frame, 'rgba8_4k_60fps_one_copy_bytes_per_second': frame * 60,
    'three_color_frames_mib': '94.921875',
    'process_pool': 'not_implemented', 'gpu_render': 'not_implemented',
    'ipc': 'not_implemented', 'memory_16gb_gate': 'not_run',
    'backend_compatibility': 'delegated_to_wgpu_no_native_adapter_implemented',
}
(root / 'docs/evidence/architecture-a55/checks.json').write_text(
    json.dumps(report, indent=2) + '\n')
print(json.dumps(report, indent=2))
