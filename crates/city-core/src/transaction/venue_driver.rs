//! Root adapter: original-pool changes and usage metadata prepare/publish together.
use super::*;
use crate::{venue_capacity as v, venue_runtime::{ActivityFact, ActivityFacts, Command, Runtime, UseCandidate}};

fn non_venue_ids(runtime: &Runtime, resources: &[Resource]) -> ([u32; MAX_RESOURCES], usize) {
    let mut ids = [0; MAX_RESOURCES];
    let mut n = 0;
    for resource in resources {
        if !runtime.is_managed(resource.id) { ids[n] = resource.id; n += 1; }
    }
    (ids, n)
}
fn snapshot<'a>(world: &World, runtime: &'a Runtime, timeline: Option<&crate::timeline::Timeline>,
    states: &'a [v::UseState], resources: &'a [Resource], leases: &'a [Option<Lease>], ids: &'a [u32]) -> v::Snapshot<'a> {
    let completed_clock = timeline.filter(|t| t.progress.phase.is_none() && t.progress.target.is_none())
        .map(|t| v::VerifiedCompletedClock::after_root_completed_timeline(world.scope, world.revision, t.progress.last_complete));
    v::Snapshot { scope: world.scope, revision: world.revision, completed_clock,
        registry: v::VenueRegistry { venues: &runtime.opening.venues }, offers: &runtime.opening.offers,
        policies: &runtime.policies, use_states: states, resources, leases, non_venue_resource_ids: ids }
}


// Read-only legacy-mode projection, never a pool used for allocation or persistence.
// Slot indices and length stay unchanged, so original bind checks retain their shape.
struct LegacyPoolView {
    resources: [Resource; MAX_RESOURCES],
    resource_len: usize,
    leases: [Option<Lease>; MAX_LEASES],
    lease_len: usize,
    non_venue: [u32; MAX_RESOURCES],
    non_venue_len: usize,
}
pub(super) const READ_SCRATCH_BYTES: usize = 8 * 1024;
const _: () = assert!(2 * std::mem::size_of::<LegacyPoolView>() + 2 * MAX_RESOURCES * std::mem::size_of::<u32>() <= READ_SCRATCH_BYTES);
impl LegacyPoolView {
    fn new(world: &World, runtime: &Runtime, resources: &[Resource], leases: &[Option<Lease>], timed: Option<&crate::time_capacity::Runtime>) -> Result<Self, TxError> {
        if resources.len() > MAX_RESOURCES || leases.len() > MAX_LEASES { return Err(TxError::WorkingLimit); }
        // Keep the complete original identity/domain gate before any quantity filtering.
        let (ids, count) = non_venue_ids(runtime, resources);
        v::validate_opening(v::OpeningView { scope: world.scope, registry: v::VenueRegistry { venues: &runtime.opening.venues }, offers: &runtime.opening.offers, resources, non_venue_resource_ids: &ids[..count] }).map_err(|_| TxError::Resource)?;
        time_capacity_driver::validate(world, timed, resources, leases)?;
        let mut view = Self { resources: [Resource { id: 0, capacity: 0, reserved: 0, occupied: 0 }; MAX_RESOURCES], resource_len: 0, leases: [None; MAX_LEASES], lease_len: leases.len(), non_venue: [0; MAX_RESOURCES], non_venue_len: 0 };
        for resource in resources {
            if timed.is_some_and(|s| s.manages(resource.id)) { continue; }
            view.resources[view.resource_len] = *resource; view.resource_len += 1;
            if !runtime.is_managed(resource.id) { view.non_venue[view.non_venue_len] = resource.id; view.non_venue_len += 1; }
        }
        for (i, lease) in leases.iter().enumerate() {
            if lease.is_some_and(|l| timed.is_some_and(|s| s.manages(l.resource))) { continue; }
            view.leases[i] = *lease;
        }
        Ok(view)
    }
    fn snapshot<'a>(&'a self, world: &World, runtime: &'a Runtime, timeline: Option<&crate::timeline::Timeline>, states: &'a [v::UseState]) -> v::Snapshot<'a> {
        snapshot(world, runtime, timeline, states, &self.resources[..self.resource_len], &self.leases[..self.lease_len], &self.non_venue[..self.non_venue_len])
    }
}

/// Static domain exclusion plus dynamic bidirectional token checks cover every internal
/// effect path as well as the public commands. Original non-managed cleanup remains valid.
pub(super) fn validate(world: &World, resources: &[Resource], leases: &[Option<Lease>],
    timeline: Option<&crate::timeline::Timeline>, candidate: Option<&UseCandidate>, timed: Option<&crate::time_capacity::Runtime>) -> Result<(), TxError> {
    let Some(runtime) = world.venue.as_deref() else { return Ok(()); };
    validate_domain_bindings(world, world.boot.as_ref(), world.exchange.as_ref(), world.construction.as_ref())?;
    if resources.len() > MAX_RESOURCES || leases.len() > MAX_LEASES { return Err(TxError::WorkingLimit); }
    runtime.validate_population(world.scope, world.housing.as_deref().map(|h| &h.opening)).map_err(|_| TxError::InvalidOperation)?;
    if runtime.opening.limits.len() != runtime.opening.venues.len() { return Err(TxError::Resource); }
    for (index, limit) in runtime.opening.limits.iter().enumerate() {
        if limit.maximum_people == 0 || !runtime.is_managed(limit.resource_id)
            || runtime.opening.limits[..index].iter().any(|old| old.resource_id == limit.resource_id)
            || resources.iter().find(|r| r.id == limit.resource_id).is_none_or(|r| r.capacity != limit.maximum_people) {
            return Err(TxError::Resource);
        }
    }
    for venue in runtime.opening.venues.iter() {
        world.account_handle(venue.owner).map_err(|_| TxError::Unauthorized)?;
        world.account_handle(venue.operator).map_err(|_| TxError::Unauthorized)?;
        if runtime.policies.iter().find(|p| p.venue_id == venue.venue_id).is_none_or(|p|
            p.owner != venue.owner || p.operator != venue.operator || p.permit_version != venue.permit_version || !p.open || p.revoked) {
            return Err(TxError::InvalidOperation);
        }
        let registered_maintenance=venue.purpose.0==crate::maintenance_authority::PURPOSE
            &&world.facility_work.as_deref().is_some_and(|r|r.maintenance.is_some()
                &&r.state.opening().contracts.iter().flatten().any(|c|
                    c.role==crate::facility_work::Role::ElectricalMaintenance
                    &&c.employer_account==venue.owner&&c.employer_account==venue.operator
                    &&runtime.opening.offers.iter().any(|o|o.use_id==c.venue_use_id&&o.venue_id==venue.venue_id)));
        if !matches!(venue.purpose.0, 1 | 2)&&!registered_maintenance { return Err(TxError::InvalidOperation); }

    }
    let states = candidate.map_or(&runtime.states[..], |c| &c.states[..]);
    let facts = candidate.map_or(&runtime.facts[..], |c| &c.facts[..]);
    runtime.validate_fact_tables(states, facts).map_err(|_| TxError::InvalidOperation)?;
    if world.town.as_ref().and_then(|market| market.state.exit).is_some_and(|exit|
        exit.phase == crate::town_runtime::ExitPhase::Closed && states.iter().any(|state|
            state.accepted_operator == exit.company_account && matches!(state.status, v::UseStatus::Reserved | v::UseStatus::Actual))) {
        return Err(TxError::InvalidOperation);
    }
    let view = LegacyPoolView::new(world, runtime, resources, leases, timed)?;
    v::validate_recovery(view.snapshot(world, runtime, timeline, states))
        .map_err(map_error)?;
    Ok(())
}
/// Validate staged binding metadata too: an unoccupied newly conflicting binding must
/// fail before commit even if original counter/lease sums still happen to agree.
pub(super) fn validate_domain_bindings(world: &World, boot: Option<&crate::boot::Boot>,
    exchange: Option<&crate::exchange::Exchange>, construction: Option<&crate::construction::Project>) -> Result<(), TxError> {
    let Some(runtime) = world.venue.as_deref() else { return Ok(()); };
    for venue in runtime.opening.venues.iter() {
        if boot.is_some_and(|b| b.resources.contains(&venue.resource_id))
            || exchange.is_some_and(|e| e.edges.iter().any(|edge| edge.resource == venue.resource_id))
            || construction.is_some_and(|p| [p.site_resource, p.operating_resource, p.waste_receiver_resource].contains(&venue.resource_id)) {
            return Err(TxError::Resource);
        }
    }
    Ok(())
}

fn map_error(error: v::PlanError) -> TxError {
    match error {
        v::PlanError::Unauthorized => TxError::Unauthorized,
        v::PlanError::Overflow => TxError::Overflow,
        v::PlanError::Quota => TxError::WorkingLimit,
        // Conservative admission rejection is never a measured physical shortage.
        _ => TxError::InvalidOperation,
    }
}

pub(super) fn prepare(engine: &Engine, caller: Caller, env: &Envelope, use_id: u64, command: Command,
    resources: &mut [Resource], leases: &mut [Option<Lease>], next_token: &mut u64)
    -> Result<(UseCandidate, Option<CapacityToken>), TxError> {
    if env.actions.len() != 1 { return Err(TxError::InvalidOperation); }
    let world = &engine.world;
    let runtime = world.venue.as_deref().ok_or(TxError::InvalidOperation)?;
    let timeline = engine.timeline.as_ref().ok_or(TxError::InvalidOperation)?;
    if timeline.progress.phase.is_some() || timeline.progress.target.is_some() { return Err(TxError::InvalidOperation); }
    let tick = timeline.progress.last_complete;
    let offer = *runtime.opening.offers.iter().find(|offer| offer.use_id == use_id).ok_or(TxError::InvalidOperation)?;
    let venue = *runtime.opening.venues.iter().find(|venue| venue.venue_id == offer.venue_id).ok_or(TxError::Resource)?;
    let (kind, permission, occurred_at, activity) = match command {
        Command::Accept => (v::RequestKind::Accept, v::EffectivePermission::ReserveOffer, None, None),
        Command::CancelReserved => (v::RequestKind::CancelReserved, v::EffectivePermission::CancelReservation, None, None),
        Command::Enter { event_id, token_number, occurred_at } => (v::RequestKind::Enter, v::EffectivePermission::EnterVenue, Some(occurred_at), Some((event_id, token_number))),
        Command::SafeExit { event_id, token_number, occurred_at } => (v::RequestKind::SafeExit, v::EffectivePermission::SafeExitVenue, Some(occurred_at), Some((event_id, token_number))),
    };
    if matches!(command, Command::Accept) && (market_is_suspended(world.town.as_ref().map(|m| &m.state), offer.operator)
        || market_is_exiting(world.town.as_ref().map(|m| &m.state), offer.operator)) { return Err(TxError::InvalidOperation); }
    if let Some((event_id, token_number)) = activity {
        if !matches!(caller, Caller::System) || occurred_at != Some(tick) || event_id == 0 {
            return Err(TxError::Unauthorized);
        }
        let state = runtime.states.iter().find(|state| state.use_id == use_id).ok_or(TxError::Lease)?;
        if state.token.number != token_number || runtime.facts.iter().flat_map(|row| [row.entered, row.exited]).flatten().any(|old| old.event_id == event_id) {
            return Err(TxError::InvalidOperation);
        }
    }
    let actor = match caller { Caller::System => v::RootActor::System, Caller::Account(id) => v::RootActor::Account(id) };
    let request = v::Request { use_id, operation_id: env.operation_id, kind, occurred_at };
    if engine.owned_capacity_bytes().checked_add(UseCandidate::payload_bytes() + READ_SCRATCH_BYTES).is_none_or(|n| n > ENGINE_BYTE_BUDGET) { return Err(TxError::WorkingLimit); }
    let before_view = LegacyPoolView::new(world, runtime, &engine.resources, &engine.leases, engine.time_capacity.as_deref())?;
    let before = before_view.snapshot(world, runtime, Some(timeline), &runtime.states);
    if matches!(command, Command::CancelReserved) {
        let state = runtime.states.iter().find(|state| state.use_id == use_id).ok_or(TxError::Lease)?;
        if !matches!(caller, Caller::System) && caller.key() != state.actor { return Err(TxError::Unauthorized); }
    }
    // Original operation IDs form the transaction-event namespace. Raw host activity IDs
    // are independently retained in facts and may never be replayed under another operation.
    let event_source = v::SourceRef { kind: v::SourceKind::GameEvent, scope: world.scope,
        revision: world.revision, operation_id: env.operation_id, event_id: env.operation_id, source_id: 0 };
    let token = runtime.states.iter().find(|state| state.use_id == use_id).map(|state| state.token);
    let event = v::VerifiedRootEvent::after_root_event_validation(request, before, actor, permission, venue, offer,
        event_source, tick, token);
    let planned = v::plan(before, request, &event).map_err(map_error)?;
    if engine.owned_capacity_bytes().checked_add(UseCandidate::payload_bytes() + READ_SCRATCH_BYTES).is_none_or(|n| n > ENGINE_BYTE_BUDGET) {
        return Err(TxError::WorkingLimit);
    }
    let mut out = UseCandidate::from_runtime(runtime);
    let mut created_token = None;
    match planned.pool {
        v::PoolDelta::Reserve { resource_id, actor, units } => {
            let resource = resources.iter_mut().find(|row| row.id == resource_id).ok_or(TxError::Resource)?;
            let reserved = resource.reserved.checked_add(units).ok_or(TxError::Overflow)?;
            if reserved.checked_add(resource.occupied).ok_or(TxError::Overflow)? > resource.capacity { return Err(TxError::InvalidOperation); }
            let slot = leases.iter_mut().find(|row| row.is_none()).ok_or(TxError::WorkingLimit)?;
            let token = *next_token;
            *next_token = token.checked_add(1).ok_or(TxError::Overflow)?;
            if token == 0 { return Err(TxError::Overflow); }
            *slot = Some(Lease { token, actor, resource: resource_id, units, occupied: false });
            resource.reserved = reserved;
            let after_view = LegacyPoolView::new(world, runtime, resources, leases, engine.time_capacity.as_deref())?;
            let after = after_view.snapshot(world, runtime, Some(timeline), &runtime.states);
            let bound = v::bind_reserved_offer(before, after, request, &event).map_err(map_error)?;
            let v::UseStateDelta::Insert(state) = bound.metadata else { return Err(TxError::InvalidOperation); };
            out.states.try_push(state).map_err(|_| TxError::WorkingLimit)?;
            created_token = Some(state.token);
        }
        v::PoolDelta::Occupy { token } | v::PoolDelta::Release { token } => {
            let slot = leases.iter_mut().find(|row| row.is_some_and(|l| l.token == token.number)).ok_or(TxError::Lease)?;
            let lease = slot.ok_or(TxError::Lease)?;
            let resource = resources.iter_mut().find(|row| row.id == lease.resource).ok_or(TxError::Resource)?;
            if matches!(planned.pool, v::PoolDelta::Occupy { .. }) {
                resource.reserved = resource.reserved.checked_sub(lease.units).ok_or(TxError::Lease)?;
                resource.occupied = resource.occupied.checked_add(lease.units).ok_or(TxError::Overflow)?;
                *slot = Some(Lease { occupied: true, ..lease });
            } else {
                if lease.occupied { resource.occupied = resource.occupied.checked_sub(lease.units).ok_or(TxError::Lease)?; }
                else { resource.reserved = resource.reserved.checked_sub(lease.units).ok_or(TxError::Lease)?; }
                *slot = None;
            }
            let v::UseStateDelta::Transition { use_id, from, to, event, occurred_at } = planned.metadata else { return Err(TxError::InvalidOperation); };
            let state = out.states.iter_mut().find(|row| row.use_id == use_id && row.status == from).ok_or(TxError::InvalidOperation)?;
            state.status = to;
            state.last_event = event;
            match to {
                v::UseStatus::Actual => state.entered_at = Some(occurred_at),
                v::UseStatus::Released => state.released_at = Some(occurred_at),
                _ => {},
            }
        }
        v::PoolDelta::None => return Err(TxError::InvalidOperation),
    }
    if let Some((event_id, _)) = activity {
        let fact = ActivityFact { event_id, operation_id: env.operation_id, occurred_at: tick, transaction_source: event_source };
        match command {
            Command::Enter { .. } => out.facts.try_push(ActivityFacts { use_id, entered: Some(fact), exited: None }).map_err(|_| TxError::WorkingLimit)?,
            Command::SafeExit { .. } => {
                let row = out.facts.iter_mut().find(|row| row.use_id == use_id).ok_or(TxError::InvalidOperation)?;
                if row.exited.replace(fact).is_some() { return Err(TxError::InvalidOperation); }
            }
            _ => return Err(TxError::InvalidOperation),
        }
    }
    validate(world, resources, leases, Some(timeline), Some(&out), engine.time_capacity.as_deref())?;
    Ok((out, created_token))
}
