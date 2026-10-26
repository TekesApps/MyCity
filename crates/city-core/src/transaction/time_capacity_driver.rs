//! Sole original-pool writer for explicitly registered timed resources.
use super::*;
use crate::time_capacity::{Command, Registration, Runtime, Window};

fn failure(_: &'static str) -> TxError { TxError::InvalidOperation }
fn stable_tick(engine: &Engine) -> Result<u64, TxError> {
    let timeline = engine.timeline.as_ref().ok_or(TxError::InvalidOperation)?;
    if timeline.progress.phase.is_some() || timeline.progress.target.is_some() { return Err(TxError::InvalidOperation); }
    Ok(timeline.progress.last_complete)
}

pub(super) fn validate_bindings(world: &World, state: &Runtime, boot: Option<&crate::boot::Boot>, exchange: Option<&crate::exchange::Exchange>, construction: Option<&crate::construction::Project>) -> Result<(), TxError> {
    for r in state.registrations.iter() {
        world.account_handle(r.owner).map_err(|_| TxError::Unauthorized)?;
        world.account_handle(r.operator).map_err(|_| TxError::Unauthorized)?;
        if world.venue.as_deref().is_some_and(|v| v.opening.venues.iter().any(|venue|
            r.canonical == u128::from(venue.venue_id) || venue.aliases.iter().flatten().any(|a| r.canonical == u128::from(*a))
            || r.aliases.iter().flatten().any(|a| *a == venue.venue_id || venue.aliases.contains(&Some(*a))))) { return Err(TxError::Resource); }

        if world.venue.as_deref().is_some_and(|v| v.is_managed(r.resource))
            || world.continuous_supply.as_deref().is_some_and(|s| s.owns_resource(r.resource))
            || boot.is_some_and(|b| b.resources.contains(&r.resource))
            || exchange.is_some_and(|e| e.edges.iter().any(|edge| edge.resource == r.resource))
            || construction.is_some_and(|p| [p.site_resource, p.operating_resource, p.waste_receiver_resource].contains(&r.resource)) {
            return Err(TxError::Resource);
        }
    }
    Ok(())
}

pub(super) fn validate(world: &World, state: Option<&Runtime>, resources: &[Resource], leases: &[Option<Lease>]) -> Result<(), TxError> {
    if let Some(state) = state {
        for (i, lease) in leases.iter().enumerate() {
            if let Some(lease) = lease {
                if lease.token == 0 || lease.units == 0 || !resources.iter().any(|r| r.id == lease.resource)
                    || leases[..i].iter().flatten().any(|old| old.token == lease.token) { return Err(TxError::Lease); }
            }
        }
        validate_bindings(world, state, world.boot.as_ref(), world.exchange.as_ref(), world.construction.as_ref())?;
        state.validate(resources, leases).map_err(failure)?;
        for lease in leases.iter().flatten().filter(|l| state.manages(l.resource)) {
            world.account_handle(lease.actor).map_err(|_| TxError::Unauthorized)?;
        }
    }
    Ok(())
}

fn refresh(state: &Runtime, resources: &mut [Resource], leases: &[Option<Lease>]) -> Result<(), TxError> {
    for registration in state.registrations.iter() {
        let p = state.project(registration, leases).map_err(failure)?;
        let r = resources.iter_mut().find(|r| r.id == registration.resource).ok_or(TxError::Resource)?;
        r.reserved = p.reserved_peak;
        r.occupied = p.occupied;
    }
    state.validate(resources, leases).map_err(failure)
}

pub(super) fn prepare(engine: &Engine, caller: Caller, env: &Envelope, command: Command,
    resources: &mut [Resource], leases: &mut [Option<Lease>], next_token: &mut u64,
) -> Result<(Box<Runtime>, Option<CapacityToken>), TxError> {
    if env.actions.len() != 1 { return Err(TxError::InvalidOperation); }
    if engine.owned_capacity_bytes().checked_add(Runtime::max_owned_bytes()).is_none_or(|n| n > ENGINE_BYTE_BUDGET) { return Err(TxError::WorkingLimit); }
    let mut state = match engine.time_capacity.as_deref() {
        Some(state) => Box::new(state.clone()),
        None if matches!(command, Command::Register { .. }) => Box::new(Runtime::new().map_err(failure)?),
        None => return Err(TxError::Resource),
    };
    let mut output = None;
    match command {
        Command::Register { registration } => {
            if !matches!(caller, Caller::System) || registration.identity_version != 1 || registration.policy_version != 1 { return Err(TxError::Unauthorized); }
            let original = resources.iter().find(|r| r.id == registration.resource).ok_or(TxError::Resource)?;
            if original.reserved != 0 || original.occupied != 0 || leases.iter().flatten().any(|l| l.resource == original.id)
                || original.capacity != registration.nominal_capacity { return Err(TxError::Resource); }
            state.registrations.try_push(registration).map_err(|_| TxError::WorkingLimit)?;
        }
        Command::SetPolicy { alias, expected_version, effective_capacity, open } => {
            if !matches!(caller, Caller::System) { return Err(TxError::Unauthorized); }
            let r = state.registrations.iter_mut().find(|r| r.aliases.contains(&Some(alias))).ok_or(TxError::Resource)?;
            if expected_version != r.policy_version || effective_capacity > r.nominal_capacity { return Err(TxError::InvalidOperation); }
            r.policy_version = r.policy_version.checked_add(1).ok_or(TxError::Overflow)?;
            r.effective_capacity = effective_capacity;
            r.open = open;
        }
        Command::Reserve { alias, account, consumer, consumer_version, start, end, units } => {
            if !matches!(caller, Caller::System) && caller.key() != account { return Err(TxError::Unauthorized); }
            engine.world.account_handle(account).map_err(|_| TxError::Unauthorized)?;
            if market_is_suspended(engine.world.town.as_ref().map(|m| &m.state), account) || market_is_exiting(engine.world.town.as_ref().map(|m| &m.state), account) { return Err(TxError::InvalidOperation); }
            let tick = stable_tick(engine)?;
            if units == 0 || consumer == 0 || consumer_version == 0 || start < tick || start >= end { return Err(TxError::InvalidOperation); }
            let r: Registration = *state.resolve(alias).ok_or(TxError::Resource)?;
            if !r.open || state.project(&r, leases).map_err(failure)?.pending_conflict { return Err(TxError::Capacity); }
            let slot = leases.iter_mut().find(|l| l.is_none()).ok_or(TxError::Lease)?;
            let token = *next_token;
            *next_token = token.checked_add(1).ok_or(TxError::Overflow)?;
            state.windows.try_push(Window { token, consumer, consumer_version, identity_version: r.identity_version, start, end, accepted_operation: env.operation_id }).map_err(|_| TxError::WorkingLimit)?;
            *slot = Some(Lease { token, actor: account, resource: r.resource, units, occupied: false });
            let p = state.project(&r, leases).map_err(failure)?;
            if p.pending_conflict { return Err(TxError::Capacity); }
            output = Some(CapacityToken { world: engine.world.scope.world(), branch: engine.world.scope.branch(), number: token });
        }
        Command::Enter { token, occurred_at } | Command::Exit { token, occurred_at } => {
            if !matches!(caller, Caller::System) || occurred_at != stable_tick(engine)? { return Err(TxError::Unauthorized); }
            let index = leases.iter().position(|l| l.is_some_and(|l| l.token == token)).ok_or(TxError::Lease)?;
            let lease = leases[index].ok_or(TxError::Lease)?;
            let r = *state.registrations.iter().find(|r| r.resource == lease.resource).ok_or(TxError::Resource)?;
            let w_index = state.windows.iter().position(|w| w.token == token).ok_or(TxError::Lease)?;
            let window = state.windows[w_index];
            if matches!(command, Command::Enter { .. }) {
                if lease.occupied || !r.open || occurred_at < window.start || occurred_at >= window.end { return Err(TxError::InvalidOperation); }
                let occupied = state.project(&r, leases).map_err(failure)?.occupied;
                if occupied.checked_add(lease.units).is_none_or(|n| n > r.effective_capacity) { return Err(TxError::Capacity); }
                leases[index] = Some(Lease { occupied: true, ..lease });
            } else {
                if !lease.occupied { return Err(TxError::Lease); }
                leases[index] = None;
                remove_window(&mut state, w_index);
            }
        }
        Command::Cancel { token } => {
            let index = leases.iter().position(|l| l.is_some_and(|l| l.token == token)).ok_or(TxError::Lease)?;
            let lease = leases[index].ok_or(TxError::Lease)?;
            if lease.occupied || (!matches!(caller, Caller::System) && caller.key() != lease.actor) { return Err(TxError::Unauthorized); }
            if !state.manages(lease.resource) { return Err(TxError::Resource); }
            let w_index = state.windows.iter().position(|w| w.token == token).ok_or(TxError::Lease)?;
            leases[index] = None;
            remove_window(&mut state, w_index);
        }
    }
    refresh(&state, resources, leases)?;
    validate(&engine.world, Some(&state), resources, leases)?;
    Ok((state, output))
}
fn remove_window(state: &mut Runtime, index: usize) {
    let len = state.windows.len();
    for i in index..len - 1 { state.windows[i] = state.windows[i + 1]; }
    state.windows.resize(len - 1, <Window as crate::model_rows::Record>::inactive());
}
