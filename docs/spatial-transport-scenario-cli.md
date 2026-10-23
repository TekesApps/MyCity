# Spatial Transport Scenario CLI

This CLI is a deterministic, small-scale integration scenario for the current spatial-transport Store path. It reuses the M0 spatial, terrain, and opening fixtures and the same road/door permission actions as `tests/spatial_transport_updates.rs`. It creates one disposable test world, then advances that world's existing Timeline and market through five service days.

The scenario registers distinct road and building owners plus an administrative permit account. It closes and restores one road edge, then closes and restores one building entrance. Each update must pass the existing request, owner approval, authority approval, and day-boundary Apply actions. It settles the actual town market after each change and verifies the corresponding transport proof and sales. It also checks a missing-approval rejection, an altered candidate hash, a stale candidate, an Applied-point lost reply, cold reopen, and exact replay.

The market fixture starts with 100 funded product units. The expected sales are 40, 0, 40, 0, and 20 over days 1–5; the last day starts with 20 inventory and ends at zero. The route proof remains 4/4 for both trips on open days, while the blocked road and closed door produce their direction-specific outcomes. This avoids treating the same stock as replenished every day.

The result is limited to a three-node, single-map WALK microfixture. It can show Store-level causal behavior and recovery for these changes; it does not establish city-scale performance, 16 GB memory acceptance, other transport modes, construction-driven capacity changes, or runtime geometry/topology editing.

The executable source now exists at `crates/city-storage/examples/spatial_transport_scenario.rs`; its preserved pre-integration draft is at `/Volumes/HP P900/dev/MyCity/work/spatial-transport-scenario-current/spatial_transport_scenario.rs.pending`. It has not yet entered a Cargo build. After the root agent schedules the next compile window, the intended command is:

```sh
MYCITY_TEST_ARTIFACT_ROOT="/Volumes/HP P900/dev/MyCity/work/spatial-transport-scenario-current/evidence" \
  cargo run --locked -p mycity-storage --example spatial_transport_scenario
```

This source has not been compiled or run. The command is a planned invocation, not a validation result. Preserve the generated world, proofs, receipts, and emitted metrics for review; do not describe this scenario as passed until the root agent runs the exact source and confirms its assertions against the resulting evidence.
