# v0.5.167 shared broker admission and recovery — finite result

Issue #104 gains three actual production AdaptiveSession/Sim/Worker/Broker/Store tests.
No runtime code or original normal64MiB/role8MiB allowance changes in this increment.

## Verified contract

A reaches a real page boundary retaining its lease and unfinished work. B attempts registration
into that broker and gets exactly `normal_broker_session_busy`. Its definitely-unallocated
startup owner retires without grants or returns. A retains lease/work and stays paused without
Continue. After A completes and original retirement, B registers and finishes in sequence.
Rejection leaves grants1/returns0 unchanged; final grants2/returns2 fully restore the pool.
This proves single-session isolation and sequential reuse, not concurrent Sim/Worker execution.

Boundary cancel drains the original result. Necessary Job remains at time0; original family
retires before credit return. Cold Store reopen retains the obligation; an explicit fresh
automatic attempt finishes at time30, then returns original credit through retirement.

The fault case kills only its own known Sim, confirms OS termination, and encounters actual
Broken pipe during Continue. ControlUncertain forbids resending that decision. Original owner
retirement restores pool67108864B. Cold recovery retains stable Ticket world/branch/job/base
revision/field version/interval/rules/source/input/checkpoint identity. Execution attempt,
generation and incarnation legitimately change1 to2 for a fresh explicit attempt. It finishes
without erasing the necessary Job or business time. Cancel recovery also restores full credit.

Before execution, advisor found the initial source draft incorrectly assumed two concurrent
Sim registrations. It was corrected to rejection/sequential reuse; no runtime failure is hidden.
Future concurrency retains one Sim unique writer plus bounded Worker slots with per-slot
identity, lifecycle/credit and serial validated commits. That capability remains unimplemented.

## Validation and retained originals

Mac16,1/32GiB, existing HP P900 USB/APFS target/work.366-file fixed build read-set SHA256:
`5fcb4464980f98e495631d37f8de80a6e9f8aae829ae87f021f054a5f7863eea`.
Locked/offline release build and workspace/all-target check pass.14 serial commands pass114
tests, including13 actual controls; compute13/broker23/Sim6/registry2 also pass. Ignored
subprocess helpers remain parent-invoked/separately reported. Source, ancillary fixtures and
seven fixed executable bytes match before/after. Original advisor accepts this finite result
with no new causal/credit/recovery blocker; advisor did not rerun tests.

Persistent package: `/Volumes/HP P900/dev/MyCity/work/issue104-shared-broker-20261024/`.
Actual worlds: `/Volumes/HP P900/dev/MyCity/work/p104b05-99705/`.
929 regular files/100421278 logical bytes have SHA inventory, special sockets/FIFOs separately
listed. Complete originals/source/executables/Tickets/steps/pool states/logs remain retained.
512MiB evidence/world planning allowance is not a runtime budget. No private worktree created;
unrelated WIP and older recovery evidence stay intact. Direct integration, no PR.

Actual legal synthetic inputs are not representative cities. Full104/global multidimensional
budget/fairness/speed/backpressure/same-session multi-Worker/GPU/native/real16GB/M0/long-stress
remain unaccepted. Two earlier debug stack faults remain independently tracked and unresolved
by these release tests. Original M0 frozen load and acceptance standards are unchanged.
