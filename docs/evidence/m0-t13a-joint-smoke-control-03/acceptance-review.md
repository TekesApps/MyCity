# Acceptance boundary review for attempt 03

Parent relayed the advisor review on 2026-10-22: requiring every smoke frame to have `group_capture_complete=true` was an additional gate introduced in attempt 02's `log_budget.py`, not a requirement in the frozen acceptance fixture. The new controller may use a successful full joint smoke for an eight-hour functional endurance run when the same-source proof and conservative log-capacity screen pass. Physical identity completeness, normal pressure, physical performance, and full M0 remain separately unaccepted until their own evidence passes.

The relevant source boundaries are:

- `fixtures/m0-t13a-acceptance.json:198-210` preserves the original shared log cap and storage limits; `:212-227` defines the 28,800-second endurance and same-world/business lifecycle.
- `docs/evidence/m0-t13a-typed-workspace/observe.py:248` emits the diagnostic `group_capture_complete` field. The existing hard guards remain at `:264-269` (combined log cap, pressure, swap and free-space); its report's `short_helper_peak_completeness` explicitly limits capture claims.
- `docs/evidence/m0-t13a-joint-smoke-control-02/log_budget.py:67-68` added the `all(frame.group_capture_complete)` rejection.

Attempt 03 therefore keeps every raw frame, preserves every unknown/missing value, and includes all frames (including the 44 incomplete-identity frames and 115 frames with compiler activity) in the capacity calculation. This removes the extra completeness gate only from functional-smoke eligibility. It does not certify the 02 run, mark any incomplete identity as sampled, or lower/raise any frozen pressure, swap, disk, sampling, or log threshold.
