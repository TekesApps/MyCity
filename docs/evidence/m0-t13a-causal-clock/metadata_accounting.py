"""Reproduce existing4MiB metadata bookkeeping, without claiming native credit."""
import json
from pathlib import Path
D=Path(__file__).resolve().parent;R=D.parents[2]
root=R/'work/causal-clock-l1-smoke-01/observed/model'
r=json.loads((root/'layout/report.json').read_text());m=r['manifest']
x=json.loads((D/'l1-residency.stdout').read_text());row=x['same_phase_samples'][-1];a=row['allocator']
native=m['native']['owned_capacity_bytes'];stubs=sum(t['owned_capacity_bytes'] for t in m['stubs']);pressure=m['padding_payload_total_bytes'];cow=m['cow_reserve_bytes'];external=m['external_owned_charge_bytes'];metadata=m['metadata_and_io_charge_bytes']
base=native+stubs+pressure+cow
assert base+metadata+external==m['normal_joint_budget_bytes']
assert a['activation']['grant_bytes']==base+metadata
result=dict(schema='mycity-existing-metadata-accounting-v1',source_sha256=m['source_sha256'],iteration=row['iteration'],native_owned_capacity_bytes=native,stub_owned_capacity_bytes=stubs,pressure_payload_bytes=pressure,cow_backing_bytes=cow,existing_external64MiB_bytes=external,existing_metadata_precharge_bytes=metadata,manifest_base_bytes=base,region_grant_bytes=a['activation']['grant_bytes'],actual_current_requested_bytes=a['current_requested_bytes'],requested_minus_manifest_base_bytes=a['current_requested_bytes']-base,arena_used_bytes=a['arena_used_bytes'],arena_free_bytes=a['arena_free_bytes'],bootstrap_charged_bytes=a['activation']['bootstrap_charged_bytes'],allocation_failures=a['allocation_failures'],r07_additional_credit_bytes=0,interpretation='The4MiB is a precharge within L1 grant, not another4MiB independent Vec. Domain owned includes headers/capacities; requested is allocator requested, so their residual is not a unique object inventory. Region free is already physically initialized inside this same grant. Neither unused precharge nor footprint-requested authorizes another native/observer deduction.',next_required='Exact controlled owner/storage/lifetime, exclusion of existing64MiB/metadata/COW/role diag, and no missing replacement bytes before any versioned R07 substitution',normal='failed observed complete same-phase footprint; not relabeled not_run')
print(json.dumps(result,indent=2))
