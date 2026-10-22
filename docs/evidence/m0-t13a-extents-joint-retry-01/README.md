# R07 extent joint L1 controller retry

This retry is a new, uniquely named controller/evidence directory. The failed
first attempt in `m0-t13a-extents-joint` remains as historical controller
failure evidence and raw VM captures. The primary owner has already completed
the two historical cold restores and, after advisor review, retired that
attempt's four pressure/reserve files. The original directory is therefore not
a directly rerunnable full snapshot set. This retry does not alter its
remaining components and makes no claim that those old generations remain
fully recoverable.

The retry reuses the exact integrated source fingerprint `32c6e61b…`, frozen
source archive and fixed CLI from the first directory. The archive is a
hardlink to the original frozen file; the executable path and SHA remain those
recorded in the original `source.json`. No Cargo command or binary rebuild is
part of this retry preparation. A new live run, if later authorized by the
primary owner, writes only to
`work/m0-t13a-extents-joint-l1-retry-01` and keeps the original 8 GiB retention
ceiling and every original workload and memory threshold.

Preflight trusts the original frozen archive as the source of truth: it checks
the archive's recorded SHA, recomputes the original sorted-entry length-prefixed
stream across all 231 entries, and verifies the fixed CLI hash and embedded
`32c…` fingerprint. The only current-checkout files compared byte-for-byte are
the two input fixtures used by this workload. Current non-fixture Rust sources
are deliberately not compared because the canonical checkout has since
advanced beyond the frozen executable. `retry-preflight.json` records these
checks and the SHA-256 of the runner, controller, topology validator,
preflight, and analyzer.

The controller now treats a missing `r07-state.json` during R07 refresh as an
unavailable observation: it waits for a valid context before and after each
`vmmap` capture, and requires same phase, epoch, and list hash at both endpoints.
Missing context can never count as a passing sample. Synthetic tests cover a
realistic ack gap, an ack that remains absent until the child exits, changed
phase/epoch/list endpoints, a stale immutable list whose byte total differs
from the newer ack epoch, a same-epoch length mismatch that must fail, and
waiting through a child timeout. The exception
path releases both snapshot admission markers and the provider pause, waits on
the already-launched child without signalling or restarting it, and records
terminal status plus partial captures before returning failure.

The extent baseline is no longer taken from the idle capture. After actual
generation-1 admission, the runner observes the first valid inflight context.
It binds a cross-admission pin claim only if the frozen admission event carries
R07 state whose epoch and mapped-byte count match that context and list. The
frozen 32c admission payload does not expose that state, so this retry records
`not_made` for the cross-admission claim. Each inflight `vmmap` interval still
requires identical same-phase, same-epoch, same-list endpoints; any future
bound admission state additionally retains the full physical extent identity
check. A legal idle-to-admission tail change is covered by a synthetic test and
is not compared against a pre-admission extent list.

This directory contains only a prepared retry harness. It has not run Cargo,
pressure, `native-observe`, `vmmap`, or cold-restore commands. It does not claim
normal or performance acceptance. All cold restore and retirement decisions
remain under the primary owner. Retain any retry snapshot while a restore still
depends on its bytes; the old run's cold restores are historical evidence and
do not imply its retired pressure/reserve set remains directly recoverable.
