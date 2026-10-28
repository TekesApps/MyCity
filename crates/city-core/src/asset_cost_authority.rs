//! One original construction asset in the existing town accounting state.
//! These adapters run only inside the original Prepared; no money or capacity pool.
use crate::{asset_cost as cost, construction::Project, transaction::TxError, units::{GameTime, Money, Quantity}};
use serde::{Deserialize, Serialize};

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub enum Disposal { ZeroRecovery }

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Opening {
    pub version: u32,
    pub project: u64,
    pub owner: u32,
    pub policy: cost::Profile,
    pub disposal: Disposal,
}

/// Frozen actual commission plus a single expense-consumption cursor. All amounts
/// remain cents, and the asset is indivisible; its policy stays in the original profile.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct State {
    pub(crate) commission_operation: u64,
    pub(crate) commissioned_at: u64,
    pub(crate) basis: Money,
    pub(crate) stopped_at: Option<u64>,
    pub(crate) charged_at: u64,
    pub(crate) charged: Money,
    pub(crate) charge_operation: u64,
    pub(crate) stop_operation: u64,
    pub(crate) last_change_operation: u64,
    #[serde(skip)]
    pub(crate) ready: bool,
    pub(crate) subsequent_expense: Money,
    pub(crate) charged_subsequent_expense: Money,
    pub(crate) retirement_operation: u64,
    pub(crate) retired_at: Option<u64>,
    pub(crate) retirement_cost: Money,
    pub(crate) charged_retirement_cost: Money,
}

// Fixed payload is allocated only for a registered asset and charged to Engine ownership.
const _: () = assert!(std::mem::size_of::<State>() <= 192);
const _: () = assert!(std::mem::size_of::<Opening>() <= 128);

impl Opening {
    pub(crate) fn validate(&self) -> Result<(), &'static str> {
        if self.version != 2 || self.project == 0 || self.owner == 0
            || self.policy.revision != cost::REVISION
            || self.policy.method != cost::Method::StraightLine
            || self.policy.clock != cost::ClockKind::LocalDevelopment
            || self.policy.source.id == 0 || self.policy.source.revision == 0
            || self.policy.life_seconds == 0 || self.policy.cost_quantum.value() != 1000 {
            return Err("asset_capital_policy");
        }
        Ok(())
    }
}

fn invalid(_: impl std::fmt::Debug) -> TxError { TxError::InvalidOperation }

/// Reconstruct the immutable whole-origin cost curve, not a new actual delivery.
/// Subtracting the durable charged cursor below prevents re-capitalization/re-expense.
fn cumulative(opening: Opening, state: State, at: u64) -> Result<Money, TxError> {
    let end = state.stopped_at.map_or(at, |stop| at.min(stop));
    let elapsed = end.checked_sub(state.commissioned_at).ok_or(TxError::InvalidOperation)?;
    let time = |n| GameTime::new(i64::try_from(n).map_err(|_| TxError::Overflow)?).map_err(invalid);
    let original = cost::Snapshot::new_delivery(cost::ActualDelivery {
        lot: opening.project, asset: opening.project, owner: opening.owner,
        capital: cost::Source { id: state.commission_operation, revision: 1 },
        delivery: cost::Source { id: state.commission_operation, revision: 1 },
        source_snapshot: state.commission_operation,
        quantity: Quantity::new(1000).map_err(invalid)?, basis: state.basis,
        at: time(state.commissioned_at)?, profile: opening.policy, lease: None,
    }).map_err(invalid)?;
    if at == state.commissioned_at { return Ok(Money::ZERO); }
    let candidate = cost::plan(&original, cost::Expected {
        revision: original.revision(), source_snapshot: original.source_snapshot(),
        receipt: cost::Source { id: state.commission_operation, revision: 1 },
    }, cost::Action::Accrue { to: time(at)?, development_seconds: elapsed }).map_err(invalid)?;
    state.basis.checked_sub(candidate.against(&original).map_err(invalid)?.carrying().map_err(invalid)?).map_err(invalid)
}

pub(crate) fn validate_state(opening: Option<Opening>, state: Option<State>) -> Result<(), &'static str> {
    match (opening, state) {
        (None, None) => Ok(()),
        (None, Some(_)) => Err("asset_capital_unbound"),
        (Some(p), None) => p.validate(),
        (Some(p), Some(s)) => {
            p.validate()?;
            if s.commission_operation == 0 || s.last_change_operation == 0
                || s.stopped_at.is_some() != (s.stop_operation != 0) || s.charged_at < s.commissioned_at
                || s.charged_subsequent_expense > s.subsequent_expense
                || s.stopped_at.is_some_and(|at| at < s.commissioned_at)
                || (s.charge_operation == 0 && (s.charged_at != s.commissioned_at || s.charged != Money::ZERO))
                || cumulative(p, s, s.charged_at).map_err(|_| "asset_capital_curve")? != s.charged {
                return Err("asset_capital_cursor");
            }
            if s.retired_at.is_some() != (s.retirement_operation != 0)
                || s.charged_retirement_cost > s.retirement_cost
                || s.retired_at.is_some_and(|at| s.stopped_at.is_none_or(|stop| stop>at)) {
                return Err("asset_capital_retirement_cursor");
            }
            let expected = match s.retired_at {
                Some(at) => s.basis.checked_sub(cumulative(p,s,at).map_err(|_|"asset_capital_retirement_curve")?).map_err(|_|"asset_capital_retirement_cost")?,
                None => Money::ZERO,
            };
            if s.retirement_cost != expected { return Err("asset_capital_retirement_cost"); }
            Ok(())
        }
    }
}

/// Called after a successful construction candidate, before that Prepared commits.
pub(crate) fn construction_transition(opening: Opening, state: &mut Option<State>, before: &Project,
    after: &Project, operation: u64, timeline: Option<&crate::timeline::Timeline>) -> Result<(), TxError> {
    opening.validate().map_err(invalid)?;
    // Later actual demolition/waste costs are expenses, never a rewrite of the
    // commissioned origin and never a second cash payment. Consume them at day close.
    if let Some(s) = state.as_mut() {
        let extra = after.spent.checked_sub(before.spent).map_err(invalid)?;
        s.subsequent_expense = s.subsequent_expense.checked_add(extra).map_err(invalid)?;
        if extra > Money::ZERO { s.last_change_operation = operation; }
    }
    // Only the original successful demolition Close, after accepted waste and site
    // release, retires this indivisible asset. Withdrawal alone never does so.
    if before.phase == crate::construction::Phase::Demolition && after.phase == crate::construction::Phase::Closed {
        if let Some(s) = state.as_mut() {
            let t=timeline.ok_or(TxError::InvalidOperation)?;
            if t.progress.phase.is_some() || t.progress.target.is_some() || !after.waste_accepted
                || s.stopped_at.is_none() || s.retired_at.is_some() || operation==0 {
                return Err(TxError::InvalidOperation);
            }
            let at=t.progress.last_complete;
            s.retirement_cost=s.basis.checked_sub(cumulative(opening,*s,at)?).map_err(invalid)?;
            s.retired_at=Some(at);
            s.retirement_operation=operation;
            s.last_change_operation=operation;
        }
    }
    // Filing/review does not recognize capital and keeps its existing clock contract.
    if before.operating_published == after.operating_published { return Ok(()); }
    let t = timeline.ok_or(TxError::InvalidOperation)?;
    if t.progress.phase.is_some() || t.progress.target.is_some() || operation == 0
        || before.id != opening.project || after.id != opening.project
        || before.owner != opening.owner || after.owner != opening.owner {
        return Err(TxError::InvalidOperation);
    }
    if !before.operating_published && after.operating_published {
        if state.is_some() { return Err(TxError::InvalidOperation); }
        let at = t.progress.last_complete;
        let s = State { commission_operation: operation, commissioned_at: at, basis: after.spent,
            stopped_at: None, charged_at: at, charged: Money::ZERO, charge_operation: 0,
            subsequent_expense: Money::ZERO, charged_subsequent_expense: Money::ZERO,
            stop_operation: 0, last_change_operation: operation, ready: true,
            retirement_operation:0,retired_at:None,retirement_cost:Money::ZERO,charged_retirement_cost:Money::ZERO };
        validate_state(Some(opening), Some(s)).map_err(invalid)?;
        *state = Some(s);
    }
    if before.operating_published && !after.operating_published {
        let s = state.as_mut().ok_or(TxError::InvalidOperation)?;
        if s.stopped_at.is_some() { return Err(TxError::InvalidOperation); }
        s.stopped_at = Some(t.progress.last_complete);
        s.stop_operation = operation;
        s.last_change_operation = operation;
    }
    Ok(())
}

/// Noncash cost in the SAME original day assessment as retained earnings and tax.
pub(crate) fn assess(opening: Opening, state: &mut Option<State>, at: u64, operation: u64) -> Result<Money, TxError> {
    validate_state(Some(opening), *state).map_err(invalid)?;
    let Some(s) = state.as_mut() else { return Ok(Money::ZERO); };
    if at < s.charged_at || operation == 0 || operation == s.charge_operation {
        return Err(TxError::InvalidOperation);
    }
    let total = cumulative(opening, *s, at)?;
    let delta = total.checked_sub(s.charged).map_err(invalid)?
        .checked_add(s.subsequent_expense.checked_sub(s.charged_subsequent_expense).map_err(invalid)?).map_err(invalid)?
        .checked_add(s.retirement_cost.checked_sub(s.charged_retirement_cost).map_err(invalid)?).map_err(invalid)?;
    s.charged_retirement_cost = s.retirement_cost;
    s.charged = total;
    s.charged_subsequent_expense = s.subsequent_expense;
    s.charged_at = at;
    s.charge_operation = operation;
    s.last_change_operation = operation;
    Ok(delta)
}

/// Compact original-writer facts; the original record already owns operation,
/// actor and revision. Digests bind registration and the complete durable result.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Context { version:u32, binding:[u8;32], after:[u8;32], event:Event }
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag="kind",rename_all="snake_case",deny_unknown_fields)]
enum Event {
    Commission { at:u64, basis:Money },
    Stop { at:u64 },
    Expense { total:Money },
    Assessment { at:u64, depreciation:Money, expense:Money, retirement:Money },
    Retirement { at:u64, cost:Money },
}
const _: () = assert!(std::mem::size_of::<Option<Context>>() <= 120);
fn digest(value: &impl Serialize)->Result<[u8;32],TxError> {
    use sha2::{Digest,Sha256}; use std::io::{self,Write};
    struct Hash(Sha256);
    impl Write for Hash { fn write(&mut self,b:&[u8])->io::Result<usize>{self.0.update(b);Ok(b.len())} fn flush(&mut self)->io::Result<()>{Ok(())} }
    let mut hash=Hash(Sha256::new());serde_json::to_writer(&mut hash,value).map_err(invalid)?;Ok(hash.0.finalize().into())
}
fn state_digest(mut state:State)->Result<[u8;32],TxError>{state.ready=false;digest(&("asset-capital-state-v3",state))}
fn binding(scope:crate::arena::Scope,opening:Opening)->Result<[u8;32],TxError>{digest(&("asset-capital-registration-v3",scope,opening))}
pub(crate) fn context(scope:crate::arena::Scope,opening:Option<Opening>,before:Option<State>,after:Option<State>,operation:u64)->Result<Option<Context>,TxError>{
    if before==after{return Ok(None);}
    let opening=opening.ok_or(TxError::InvalidOperation)?;let after=after.ok_or(TxError::InvalidOperation)?;
    validate_state(Some(opening),Some(after)).map_err(invalid)?;
    if operation==0 || after.last_change_operation!=operation{return Err(TxError::InvalidOperation);}
    let event=if let Some(mut expected)=before {
        let event=if after.retirement_operation==operation && expected.retired_at.is_none() {
            expected.retirement_operation=operation;expected.retired_at=after.retired_at;expected.retirement_cost=after.retirement_cost;
            Event::Retirement{at:after.retired_at.ok_or(TxError::InvalidOperation)?,cost:after.retirement_cost}
        } else if after.stop_operation==operation && expected.stopped_at.is_none() {
            expected.stop_operation=operation;expected.stopped_at=after.stopped_at;
            Event::Stop{at:after.stopped_at.ok_or(TxError::InvalidOperation)?}
        } else if after.charge_operation==operation && expected.charge_operation!=operation {
            if after.charged<expected.charged || after.charged_subsequent_expense<expected.charged_subsequent_expense || after.charged_retirement_cost<expected.charged_retirement_cost{return Err(TxError::InvalidOperation);}
            expected.charge_operation=operation;expected.charged_at=after.charged_at;expected.charged=after.charged;
            expected.charged_subsequent_expense=after.charged_subsequent_expense;expected.charged_retirement_cost=after.charged_retirement_cost;
            Event::Assessment{at:after.charged_at,depreciation:after.charged,expense:after.charged_subsequent_expense,retirement:after.charged_retirement_cost}
        } else if after.subsequent_expense>expected.subsequent_expense {
            expected.subsequent_expense=after.subsequent_expense;Event::Expense{total:after.subsequent_expense}
        } else{return Err(TxError::InvalidOperation);};
        expected.last_change_operation=operation;
        if expected!=after{return Err(TxError::InvalidOperation);}
        event
    } else {
        if after.commission_operation!=operation || after.charge_operation!=0 || after.stop_operation!=0 || after.retirement_operation!=0 || after.subsequent_expense!=Money::ZERO {return Err(TxError::InvalidOperation);}
        Event::Commission{at:after.commissioned_at,basis:after.basis}
    };
    Ok(Some(Context{version:3,binding:binding(scope,opening)?,after:state_digest(after)?,event}))
}

pub(crate) fn operations(state: Option<State>) -> [Option<u64>; 5] {
    state.map_or([None; 5], |s| [Some(s.commission_operation), (s.stop_operation != 0).then_some(s.stop_operation),
        Some(s.last_change_operation), (s.charge_operation != 0).then_some(s.charge_operation), (s.retirement_operation!=0).then_some(s.retirement_operation)])
}

/// A bounded set of original operations, not a scan/replay of an asset's full history.
pub(crate) fn check_original(scope:crate::arena::Scope,opening:Opening,state:State,index:usize,record:&crate::transaction::JournalRecord)->Result<(),TxError>{
    use crate::{construction::EngineCommand as C,transaction::Action};
    if operations(Some(state)).get(index).copied().flatten()!=Some(record.operation_id)
        || record.outcome.as_ref().is_err()
        || record.outcome.as_ref().is_ok_and(|r|r.operation_id!=record.operation_id || record.expected_revision.checked_add(1)!=Some(r.revision))
        || record.actions.iter().skip(1).any(Option::is_some) {return Err(TxError::InvalidOperation);}
    let ctx=record.asset_capital_context().ok_or(TxError::InvalidOperation)?;
    if ctx.version!=3 || ctx.binding!=binding(scope,opening)? {return Err(TxError::InvalidOperation);}
    let action=record.actions[0];
    let valid_event=match ctx.event {
        Event::Commission{..}=>matches!(action,Some(Action::Construction{command:C::AcceptInspection{accepted:true,..}})),
        Event::Stop{..}=>matches!(action,Some(Action::Construction{command:C::Start})),
        Event::Expense{..}=>matches!(action,Some(Action::Construction{..})),
        Event::Assessment{..}=>matches!(action,Some(Action::TownSettle{..}|Action::TownTransportSettle{..})),
        Event::Retirement{..}=>matches!(action,Some(Action::Construction{command:C::Close})),
    };
    if !valid_event{return Err(TxError::InvalidOperation);}
    let expected=match index {
        0=>Event::Commission{at:state.commissioned_at,basis:state.basis},
        1=>Event::Stop{at:state.stopped_at.ok_or(TxError::InvalidOperation)?},
        2=>{return if ctx.after==state_digest(state)? {Ok(())}else{Err(TxError::InvalidOperation)};},
        3=>Event::Assessment{at:state.charged_at,depreciation:state.charged,expense:state.charged_subsequent_expense,retirement:state.charged_retirement_cost},
        4=>Event::Retirement{at:state.retired_at.ok_or(TxError::InvalidOperation)?,cost:state.retirement_cost},
        _=>return Err(TxError::InvalidOperation),
    };
    if ctx.event!=expected{return Err(TxError::InvalidOperation);}Ok(())
}

/// Opaque accumulator for a single checkpoint's fixed original sources.
pub struct RestoreCheck {
    pub(crate) opening: Opening,
    pub(crate) state: State,
    pub(crate) seen: u8,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct Runtime { pub opening: Opening, pub state: Option<State> }
impl Runtime {
    pub(crate) const fn owned_bytes() -> usize { std::mem::size_of::<Self>() }
    pub(crate) const fn workspace_bytes() -> usize { Self::owned_bytes() + std::mem::size_of::<Context>() }
}
pub(crate) fn validate(world: &crate::assembly::World) -> Result<(), &'static str> {
    let Some(runtime) = world.asset_capital.as_deref() else { return Ok(()); };
    let p = runtime.opening;
    validate_state(Some(p), runtime.state)?;
    let town = world.town.as_ref().ok_or("asset_capital_town")?;
    if !town.profile.tax_rules.as_slice().iter().any(|r| r.company == p.owner) { return Err("asset_capital_tax_owner"); }
    let project = world.construction.as_ref().ok_or("asset_capital_project")?;
    if world.region.is_some() || project.id != p.project || project.owner != p.owner
        || match runtime.state { None => project.operating_published, Some(s) => project.operating_published == s.stopped_at.is_some() } {
        return Err("asset_capital_binding");
    }
    if let Some(asset) = runtime.state {
        if (project.phase == crate::construction::Phase::Closed) != asset.retired_at.is_some() { return Err("asset_capital_retirement_binding"); }
        let last_day = town.state.settled_day.checked_mul(crate::timeline::DAY).ok_or("asset_capital_day")?;
        let cursor = town.state.tax_cursors.as_slice().iter().find(|c| c.company == p.owner).ok_or("asset_capital_tax_cursor")?;
        if (asset.charge_operation == 0 && asset.commissioned_at < last_day)
            || (asset.charge_operation != 0 && (asset.charged_at != last_day || asset.charge_operation != cursor.last_operation_id)) {
            return Err("asset_capital_tax_cursor");
        }
    }
    Ok(())
}

impl crate::restore_into::Restore for Option<Box<Runtime>> {
    fn restore<'de,D:serde::Deserializer<'de>>(&mut self,d:D)->Result<(),D::Error>{
        *self=None;
        *self=Option::<Box<Runtime>>::deserialize(d)?;
        Ok(())
    }
}
