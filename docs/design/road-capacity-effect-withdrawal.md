# One-time road capacity effect withdrawal

This implementation candidate adds one irreversible withdrawal boundary for the optional, genesis-bound road-capacity effect. It does not add general road editing, reopening, rebuilding, or arbitrary capacity inputs.

A project increases only its bound open road edge after inspection is accepted. If acceptance is published at the fully settled midnight ending service day `d`, the added capacity first applies to service day `d + 1`. The project retains the original `published_at` permanently so old transport proofs can be checked against the epoch that existed on their service day.

After demolition consent, permits, and waste-receiver checks reach `Ready`, starting demolition requires the current timeline to be idle at a whole-day boundary and the town market to have settled that exact day. The same Engine transaction withdraws the project's added capacity, records `operating_withdrawn_at`, and starts demolition. A failed capacity check leaves project, resource, and timeline state unchanged. Demolition approval alone does not remove capacity.

The added capacity applies to days beginning after `published_at`, and stops applying to days beginning after `operating_withdrawn_at`. The opening revision advances by one for the added-capacity epoch and by two after permanent withdrawal; the configured edge remains open and its access permissions are unchanged. Historical proofs retain their original daily inputs and are verified from both timestamps.

Withdrawal is one-time and irreversible. A withdrawn project remains a demolition/closed project; it cannot be republished or rebuilt. While demolition is paused, already-withdrawn capacity stays withdrawn. Existing material, waste, site-release, and closure rules still apply.

Legacy projects omit `operating_withdrawn_at`; serde defaults it to `None` and omits `None` on serialization to preserve old canonical state hashes. The current candidate supports a single bound road effect. Reversible capacity schedules and multiple simultaneous road projects remain out of scope.

Run58 executed the `town_transport road-withdraw` Store scenario and exited 0. The lifecycle
covered day-one settlement/publication, day-two expanded service and approved withdrawal at the
settled day-two boundary (timestamp 172800), day-three base-capacity service, demolition work,
waste acceptance, site release, checkpoint and cold reopen. Day-two sales were 200, day-three
base sales were 40, capacity returned to 2, and the historical proof hashes remained distinct
and valid after reopen. This is the fixed scenario's functional result, not arbitrary road editing
or every failure/retry path. Run72's final core/storage all-target release test exited 0 with
source unchanged. The 64MiB full-scale process budget and actual 16GB device gate remain
unverified; formal01 ended early under the pressure guard. Evidence is retained in
`/Volumes/HP P900/dev/MyCity/work/core-road-town-precommit-01/58-town-transport-road-withdraw/`.
