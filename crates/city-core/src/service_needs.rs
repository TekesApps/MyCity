//! LS42 natural needs only. Original Housing acceptance supplies entry; original Timeline
//! drains these pages and original Store commits them. No admission, delivery or shortage API.
use crate::{arena::Scope, model_rows::{Record, Rows}};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::{io::{self, Write}, mem::size_of};

pub const VERSION: u32 = 1;
pub const MAX_HOUSEHOLDS: usize = 32;
pub const MAX_PAGE_HOUSEHOLDS: usize = 128;
pub const MAX_EFFECTS: usize = MAX_HOUSEHOLDS * 3;
pub const MAX_SERIALIZED_BYTES: usize = 64 * 1024;
pub const RESTORE_MAX_DEPTH: usize = 12;
pub const DAY: u64 = 86_400;
pub const EIGHT_AM: u64 = 28_800;

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum Profile { LocalDevelopmentNaturalNeedsV1 }
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[repr(u8)]
pub enum Kind { Medical = 0, Police = 1, Fire = 2 }
impl Kind {
    pub const ALL: [Self; 3] = [Self::Medical, Self::Police, Self::Fire];
    pub const fn period(self) -> u32 { match self { Self::Medical => 90, Self::Police | Self::Fire => 180 } }
    pub const fn index(self) -> usize { self as usize }
}
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Household {
    pub household_id: u32,
    pub source_id: u64,
    /// Fixed immutable registration lifetime only, not a general migration generation.
    pub generation: u32,
    pub people: u16,
}
impl Record for Household { fn inactive() -> Self { Self { household_id: 0, source_id: 0, generation: 0, people: 0 } } }
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Opening {
    pub version: u32,
    pub profile: Profile,
    pub scope: Scope,
    pub seed: u64,
    /// Must equal actual OpeningHousing.city_id Some(city); Scope does not identify a city.
    pub city_id: u32,
    pub households: Rows<Household, MAX_HOUSEHOLDS>,
}
impl Opening {
    pub fn validate(&self) -> Result<(), Error> {
        if self.version != VERSION || self.city_id == 0 || self.households.is_empty() { return Err(Error::Registration); }
        for (i, h) in self.households.iter().enumerate() {
            if h.household_id == 0 || h.source_id == 0 || h.generation != 1
                || (i > 0 && self.households[i - 1].household_id >= h.household_id)
                || self.households[..i].iter().any(|old| old.source_id == h.source_id) {
                return Err(Error::Registration);
            }
        }
        Ok(())
    }
    pub fn registration_fingerprint(&self) -> Result<[u8; 32], Error> {
        digest(self, b"MyCity/service-needs/registration/v1\0")
    }
}
/// Exact root-frozen token protocol; public computation grants no entry or qualification.
pub fn stable_token(scope: Scope, h: Household) -> u64 {
    let mut hash = Sha256::new();
    hash.update(b"MyCity/service-needs/household-token/v1\0");
    hash.update(scope.world().to_le_bytes());
    hash.update(scope.branch().to_le_bytes());
    hash.update(h.household_id.to_le_bytes());
    hash.update(h.source_id.to_le_bytes());
    hash.update(h.generation.to_le_bytes());
    let bytes: [u8; 32] = hash.finalize().into();
    u64::from_be_bytes(bytes[..8].try_into().expect("eight token bytes"))
}
pub fn phase(token: u64, seed: u64, kind: Kind) -> Result<u32, Error> {
    let sum = u128::from(token).checked_add(u128::from(seed)).ok_or(Error::Overflow)?
        .checked_add(31u128.checked_mul(kind as u128).ok_or(Error::Overflow)?).ok_or(Error::Overflow)?;
    u32::try_from(sum % u128::from(kind.period())).map_err(|_| Error::Overflow)
}
fn occurrence_at(day: u32) -> Result<u64, Error> {
    u64::from(day).checked_mul(DAY).and_then(|v| v.checked_add(EIGHT_AM)).ok_or(Error::Overflow)
}
fn first_day_after(token: u64, seed: u64, kind: Kind, entry: u64) -> Result<u32, Error> {
    let day = u32::try_from(entry / DAY).map_err(|_| Error::Overflow)?;
    let p = kind.period();
    let offset = phase(token, seed, kind)?.checked_add(p).ok_or(Error::Overflow)?
        .checked_sub(day % p).ok_or(Error::Overflow)? % p;
    let mut next = day.checked_add(offset).ok_or(Error::Overflow)?;
    if occurrence_at(next)? <= entry { next = next.checked_add(p).ok_or(Error::Overflow)?; }
    // Reserve representable deadline and the following occurrence before accepting entry.
    next.checked_add(1).and_then(|_| next.checked_add(p)).ok_or(Error::Overflow)?;
    Ok(next)
}
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Entry { pub at: u64, pub operation_id: u64 }
/// Exact 16-byte current-source payload. Identity comes from immutable household registration.
/// flags: 0 empty, 1 canonical unadmitted waiting, 2 canonical expired with zero delivery.
#[repr(C)]
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Current {
    pub occurrence_day: u32,
    pub deadline_day: u32,
    pub delivery_ref: u32,
    pub flags: u32,
}
impl Current {
    pub fn is_waiting(self) -> bool { self.flags == 1 }
    pub fn is_expired(self) -> bool { self.flags == 2 }
    pub fn is_empty(self) -> bool { self.flags == 0 }
}
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Summary { pub generated: u64, pub expired: u64, pub reminders: u64 }
/// Latest exact deadline remains queryable when the next cycle reuses Current.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct DeadlineResult { pub occurrence_day: u32, pub deadline_day: u32, pub operation_id: u64 }
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct HouseholdState {
    pub household_id: u32,
    pub entry: Option<Entry>,
    pub current: [Current; 3],
    pub next_days: [Option<u32>; 3],
    pub summaries: [Summary; 3],
    pub latest_deadlines: [Option<DeadlineResult>; 3],
    pub generated_operations: [u64; 3],
    pub deadline_operations: [u64; 3],
    /// Summary chain anchor is not a substitute for original Store receipt/history details.
    pub chain: [u8; 32],
    pub last_operation: u64,
}
impl Record for HouseholdState {
    fn inactive() -> Self { Self { household_id: 0, entry: None, current: [Current::default(); 3],
        next_days: [None; 3], summaries: [Summary::default(); 3], latest_deadlines: [None; 3], generated_operations: [0; 3],
        deadline_operations: [0; 3], chain: [0; 32], last_operation: 0 } }
}
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum Subphase { Deadlines, Occurrences }
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct DueRun { pub at: u64, pub next_household: u8, pub subphase: Subphase }
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Runtime {
    /// Restored fields are structural data until the root verifies original entry receipts.
    #[serde(skip)]
    ready: bool,
    opening: Opening,
    enabled_at: u64,
    covered_through: u64,
    run: Option<DueRun>,
    households: Rows<HouseholdState, MAX_HOUSEHOLDS>,
}
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Error { Registration, Invalid, Pending, SkippedDue, Stale, Overflow, Budget, UnsupportedResponsibility, Unauthenticated }
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum Event { Generated, ExpiredUnadmittedZeroDelivery, Reminder }
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Effect {
    pub household_id: u32, pub source_id: u64, pub generation: u32,
    pub kind: Kind, pub occurrence_day: u32, pub at: u64, pub event: Event,
}
impl Record for Effect {
    fn inactive() -> Self { Self { household_id: 0, source_id: 0, generation: 0,
        kind: Kind::Medical, occurrence_day: 0, at: 0, event: Event::Generated } }
}
#[derive(Clone, Debug)]
pub struct Page {
    pub at: u64,
    pub subphase: Subphase,
    pub complete: bool,
    pub effects: Rows<Effect, MAX_EFFECTS>,
}
/// Root commits next and effects with its existing Envelope/receipt, never separately.
pub(crate) struct Candidate { pub expected: [u8; 32], pub next: Box<Runtime>, pub page: Page }
impl Candidate {
    pub fn matches(&self, current: &Runtime) -> Result<bool, Error> {
        Ok(current.ready && self.next.ready && self.expected == current.fingerprint()?)
    }
}
impl Runtime {
    pub fn new(opening: Opening, at: u64) -> Result<Self, Error> {
        opening.validate()?;
        u32::try_from(at / DAY).map_err(|_| Error::Overflow)?;
        let mut households = Rows::new().map_err(|_| Error::Budget)?;
        for h in opening.households.iter() {
            let mut state = HouseholdState::inactive(); state.household_id = h.household_id;
            households.try_push(state).map_err(|_| Error::Budget)?;
        }
        let out = Self { ready: true, opening, enabled_at: at, covered_through: at, run: None, households };
        out.validate()?; Ok(out)
    }
    pub fn opening(&self) -> &Opening { &self.opening }
    pub fn is_ready(&self) -> bool { self.ready }
    /// Only the root's original-receipt restore bridge may invoke this after all checks.
    pub(crate) fn mark_authenticated(&mut self) -> Result<(), Error> { self.validate()?; self.ready = true; Ok(()) }
    pub fn covered_through(&self) -> u64 { self.covered_through }
    pub fn pending(&self) -> Option<DueRun> { self.run }
    pub fn households(&self) -> &[HouseholdState] { &self.households }
    pub fn household(&self, id: u32) -> Option<&HouseholdState> { self.households.iter().find(|h| h.household_id == id) }
    pub fn current(&self, id: u32, kind: Kind) -> Option<Current> { self.household(id).map(|h| h.current[kind.index()]) }
    pub fn summary(&self, id: u32, kind: Kind) -> Option<Summary> { self.household(id).map(|h| h.summaries[kind.index()]) }
    pub fn latest_deadline(&self, id: u32, kind: Kind) -> Option<DeadlineResult> { self.household(id).and_then(|h| h.latest_deadlines[kind.index()]) }
    pub fn registration_fingerprint(&self) -> Result<[u8; 32], Error> { self.opening.registration_fingerprint() }
    pub fn fingerprint(&self) -> Result<[u8; 32], Error> { digest(self, b"MyCity/service-needs/runtime/v1\0") }
    pub const fn owned_bytes() -> usize { size_of::<Self>() + MAX_HOUSEHOLDS * (size_of::<Household>() + size_of::<HouseholdState>()) }
    pub const fn candidate_bytes() -> usize {
        size_of::<Candidate>() + Self::owned_bytes() + MAX_EFFECTS * size_of::<Effect>()
    }
    /// Conservative raw model peak. Root decoder/native stacks/Store buffers still add to this.
    pub const fn restore_workspace_bytes() -> usize { Self::owned_bytes() + MAX_SERIALIZED_BYTES }
    pub fn next_due(&self) -> Result<Option<u64>, Error> {
        if let Some(run) = self.run { return Ok(Some(run.at)); }
        let mut next = None;
        for row in self.households.iter() {
            for (i, c) in row.current.iter().enumerate() {
                if let Some(day) = row.next_days[i] { take_min(&mut next, occurrence_at(day)?); }
                if c.is_waiting() { take_min(&mut next, occurrence_at(c.deadline_day)?); }
            }
        }
        Ok(next)
    }
    /// Trusted root only: actual successful HousingLease acceptance at a stable Timeline point.
    /// This registers limited legal occupancy, not travel arrival or a general city-entry fact.
    pub(crate) fn record_legal_entry(&self, household_id: u32, at: u64, operation_id: u64) -> Result<Candidate, Error> {
        if !self.ready { return Err(Error::Unauthenticated); }
        self.validate()?;
        if self.run.is_some() { return Err(Error::Pending); }
        if at != self.covered_through || operation_id == 0 { return Err(Error::Stale); }
        let index = self.households.iter().position(|h| h.household_id == household_id).ok_or(Error::Registration)?;
        let mut next = Box::new(self.clone());
        let reg = self.opening.households[index];
        if reg.people == 0 { return Err(Error::Registration); }
        let state = &mut next.households[index];
        if state.entry.is_none() {
            state.entry = Some(Entry { at, operation_id });
            for kind in Kind::ALL {
                state.next_days[kind.index()] = Some(first_day_after(stable_token(self.opening.scope, reg), self.opening.seed, kind, at)?);
            }
            state.last_operation = operation_id;
            state.chain = digest(&(state.chain, reg, Entry { at, operation_id }), b"MyCity/service-needs/entry/v1\0")?;
        }
        // Subsequent successful leases never reset the first entry or coverage/phase.
        next.validate()?;
        Ok(Candidate { expected: self.fingerprint()?, next, page: Page { at, subphase: Subphase::Occurrences,
            complete: true, effects: Rows::new().map_err(|_| Error::Budget)? } })
    }
    /// Root-only P5 page. at is the controlled boundary, NOT necessarily stable last_complete yet.
    /// max_households=0 is unresolved/backpressure and makes no candidate or business result.
    pub(crate) fn prepare_due(&self, at: u64, max_households: usize, operation_id: u64) -> Result<Candidate, Error> {
        if !self.ready { return Err(Error::Unauthenticated); }
        self.validate()?;
        if max_households == 0 || max_households > MAX_PAGE_HOUSEHOLDS { return Err(Error::Budget); }
        if operation_id == 0 || at < self.covered_through { return Err(Error::Stale); }
        if let Some(run) = self.run {
            if at != run.at { return Err(Error::Pending); }
        } else if self.next_due()?.is_some_and(|due| at > due) { return Err(Error::SkippedDue); }
        let mut next = Box::new(self.clone());
        // No event at this boundary: one coverage mutation, without deadline/occurrence pages.
        // Validation, fingerprint and fixed backing copy still have bounded table costs.
        if self.run.is_none() && self.next_due()?.is_none_or(|due| at < due) {
            next.covered_through = at;
            next.validate()?;
            return Ok(Candidate { expected: self.fingerprint()?, next,
                page: Page { at, subphase: Subphase::Deadlines, complete: true,
                    effects: Rows::new().map_err(|_| Error::Budget)? } });
        }
        let run = next.run.unwrap_or(DueRun { at, next_household: 0, subphase: Subphase::Deadlines });
        let start = usize::from(run.next_household);
        let end = (start + max_households).min(next.households.len());
        let mut page = Page { at, subphase: run.subphase, complete: false, effects: Rows::new().map_err(|_| Error::Budget)? };
        for index in start..end {
            let reg = next.opening.households[index];
            let row = &mut next.households[index];
            if row.entry.is_none() { continue; }
            for kind in Kind::ALL {
                let k = kind.index();
                let current = row.current[k];
                let event = match run.subphase {
                    Subphase::Deadlines if current.is_waiting() && occurrence_at(current.deadline_day)? == at => {
                        // This version has no admission/work consumer; nonzero delivery is rejected
                        // at validate, never silently reclaimed or declared zero actual work.
                        if current.delivery_ref != 0 { return Err(Error::UnsupportedResponsibility); }
                        row.current[k].flags = 2;
                        row.deadline_operations[k] = operation_id;
                        row.latest_deadlines[k] = Some(DeadlineResult { occurrence_day: current.occurrence_day,
                            deadline_day: current.deadline_day, operation_id });
                        row.summaries[k].expired = row.summaries[k].expired.checked_add(1).ok_or(Error::Overflow)?;
                        Some(Event::ExpiredUnadmittedZeroDelivery)
                    }
                    Subphase::Occurrences if row.next_days[k].is_some_and(|day| occurrence_at(day).ok() == Some(at)) => {
                        let day = row.next_days[k].ok_or(Error::Invalid)?;
                        let following = day.checked_add(kind.period()).ok_or(Error::Overflow)?;
                        following.checked_add(1).ok_or(Error::Overflow)?;
                        row.next_days[k] = Some(following);
                        if current.is_waiting() {
                            row.summaries[k].reminders = row.summaries[k].reminders.checked_add(1).ok_or(Error::Overflow)?;
                            Some(Event::Reminder)
                        } else {
                            row.current[k] = Current { occurrence_day: day, deadline_day: day.checked_add(1).ok_or(Error::Overflow)?, delivery_ref: 0, flags: 1 };
                            row.generated_operations[k] = operation_id;
                            row.deadline_operations[k] = 0;
                            row.summaries[k].generated = row.summaries[k].generated.checked_add(1).ok_or(Error::Overflow)?;
                            Some(Event::Generated)
                        }
                    }
                    _ => None,
                };
                if let Some(event) = event {
                    let effect = Effect { household_id: reg.household_id, source_id: reg.source_id, generation: reg.generation,
                        kind, occurrence_day: row.current[k].occurrence_day, at, event };
                    row.chain = digest(&(row.chain, effect, operation_id), b"MyCity/service-needs/effect/v1\0")?;
                    row.last_operation = operation_id;
                    page.effects.try_push(effect).map_err(|_| Error::Budget)?;
                }
            }
        }
        if end < next.households.len() {
            next.run = Some(DueRun { at, next_household: u8::try_from(end).map_err(|_| Error::Overflow)?, subphase: run.subphase });
        } else if run.subphase == Subphase::Deadlines {
            next.run = Some(DueRun { at, next_household: 0, subphase: Subphase::Occurrences });
        } else {
            next.run = None; next.covered_through = at; page.complete = true;
        }
        next.validate()?;
        Ok(Candidate { expected: self.fingerprint()?, next, page })
    }
    pub fn validate(&self) -> Result<(), Error> {
        self.opening.validate()?;
        if self.households.len() != self.opening.households.len() || self.covered_through < self.enabled_at {
            return Err(Error::Invalid);
        }
        let through = self.run.map_or(self.covered_through, |r| r.at);
        u32::try_from(through / DAY).map_err(|_| Error::Overflow)?;
        if let Some(run) = self.run {
            if run.at < self.covered_through || usize::from(run.next_household) >= self.households.len() { return Err(Error::Invalid); }
        }
        for (index, row) in self.households.iter().enumerate() {
            let reg = self.opening.households[index];
            if row.household_id != reg.household_id { return Err(Error::Registration); }
            let Some(entry) = row.entry else {
                let mut empty = HouseholdState::inactive(); empty.household_id = reg.household_id;
                if *row != empty { return Err(Error::Invalid); }
                continue;
            };
            if entry.operation_id == 0 || entry.at < self.enabled_at || entry.at > self.covered_through
                || reg.people == 0 || row.chain == [0; 32] || row.last_operation == 0 { return Err(Error::Invalid); }
            for kind in Kind::ALL {
                let k = kind.index(); let current = row.current[k]; let summary = row.summaries[k];
                let next_day = row.next_days[k].ok_or(Error::Invalid)?;
                let first = first_day_after(stable_token(self.opening.scope, reg), self.opening.seed, kind, entry.at)?;
                if next_day < first || (next_day - first) % kind.period() != 0 { return Err(Error::Invalid); }
                let expected_processed = u64::from((next_day - first) / kind.period());
                if summary.generated.checked_add(summary.reminders).ok_or(Error::Overflow)? != expected_processed
                    || summary.expired > summary.generated { return Err(Error::Invalid); }
                // This profile cannot admit a delivery or carry a task past deadline. A future
                // version must explicitly preserve its held responsibility, not reinterpret v1.
                if summary.reminders != 0 { return Err(Error::UnsupportedResponsibility); }
                match row.latest_deadlines[k] {
                    None if summary.expired != 0 => return Err(Error::Invalid),
                    Some(d) => {
                        if summary.expired == 0 || d.operation_id == 0 || d.occurrence_day < first
                            || (d.occurrence_day - first) % kind.period() != 0
                            || d.deadline_day != d.occurrence_day.checked_add(1).ok_or(Error::Overflow)?
                            || occurrence_at(d.deadline_day)? > through { return Err(Error::Invalid); }
                        let expected_day = first.checked_add(u32::try_from(summary.expired - 1).map_err(|_| Error::Overflow)?
                            .checked_mul(kind.period()).ok_or(Error::Overflow)?).ok_or(Error::Overflow)?;
                        if d.occurrence_day != expected_day { return Err(Error::Invalid); }
                    }
                    None => {}
                }
                if current.delivery_ref != 0 { return Err(Error::UnsupportedResponsibility); }
                if current.flags > 2 { return Err(Error::UnsupportedResponsibility); }
                if current.is_empty() {
                    if current != Current::default() || summary != Summary::default() || next_day != first
                        || row.generated_operations[k] != 0 || row.deadline_operations[k] != 0 { return Err(Error::Invalid); }
                } else {
                    if current.occurrence_day < first || (current.occurrence_day - first) % kind.period() != 0
                        || current.deadline_day != current.occurrence_day.checked_add(1).ok_or(Error::Overflow)?
                        || occurrence_at(current.occurrence_day)? > through || occurrence_at(current.occurrence_day)? <= entry.at
                        || Some(current.occurrence_day) != next_day.checked_sub(kind.period()) { return Err(Error::Invalid); }
                    let outstanding = summary.generated.checked_sub(summary.expired).ok_or(Error::Invalid)?;
                    if outstanding != u64::from(current.is_waiting()) { return Err(Error::Invalid); }
                    if row.generated_operations[k] == 0 || (current.is_waiting() && row.deadline_operations[k] != 0)
                        || (current.is_expired() && row.deadline_operations[k] == 0) { return Err(Error::Invalid); }
                    if current.is_expired() && row.latest_deadlines[k] != Some(DeadlineResult {
                        occurrence_day: current.occurrence_day, deadline_day: current.deadline_day,
                        operation_id: row.deadline_operations[k] }) { return Err(Error::Invalid); }
                    if current.is_expired() && occurrence_at(current.deadline_day)? > through { return Err(Error::Invalid); }
                }
                // Coverage is exact even at partially committed pages. An unprocessed event
                // may equal pending at, but must never be hidden behind stable coverage.
                let pending_occurrence = occurrence_at(next_day)?;
                let occurrence_processed_here = self.run.is_some_and(|r| r.subphase == Subphase::Occurrences && index < usize::from(r.next_household));
                if pending_occurrence <= self.covered_through && self.run.is_none()
                    || pending_occurrence < through
                    || (pending_occurrence == through && occurrence_processed_here) { return Err(Error::Invalid); }
                if current.is_waiting() {
                    let deadline = occurrence_at(current.deadline_day)?;
                    let deadline_processed_here = self.run.is_some_and(|r| r.subphase == Subphase::Occurrences || index < usize::from(r.next_household));
                    if deadline <= self.covered_through && self.run.is_none() || deadline < through
                        || (deadline == through && deadline_processed_here) { return Err(Error::Invalid); }
                }
            }
        }
        Ok(())
    }
}
fn take_min(slot: &mut Option<u64>, value: u64) { *slot = Some(slot.map_or(value, |old| old.min(value))); }
struct HashWriter { hash: Sha256, bytes: usize }
impl Write for HashWriter {
    fn write(&mut self, bytes: &[u8]) -> io::Result<usize> {
        self.bytes = self.bytes.checked_add(bytes.len()).ok_or_else(|| io::Error::other("service_needs_bytes"))?;
        if self.bytes > MAX_SERIALIZED_BYTES { return Err(io::Error::other("service_needs_bytes")); }
        self.hash.update(bytes); Ok(bytes.len())
    }
    fn flush(&mut self) -> io::Result<()> { Ok(()) }
}
fn digest(value: &impl Serialize, domain: &[u8]) -> Result<[u8; 32], Error> {
    let mut writer = HashWriter { hash: Sha256::new(), bytes: 0 }; writer.hash.update(domain);
    serde_json::to_writer(&mut writer, value).map_err(|_| Error::Budget)?;
    Ok(writer.hash.finalize().into())
}
