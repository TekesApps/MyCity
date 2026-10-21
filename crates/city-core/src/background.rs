//! Finite regional air obligation. Sim owns this state; workers only propose a candidate.
use crate::model_rows::{Resident, ResidentValue};
use crate::{
    air::{Input, Source, State},
    exchange::{Exchange, Kind},
    sha256,
};
use serde::{Deserialize, Serialize};
use std::mem::size_of;
pub const SIDE: u16 = 128;
pub const SOURCE_LIMIT: usize = crate::exchange::MAX_SHIPMENTS;
pub const OWNED_LIMIT: usize = 320 * 1024;
pub const ASSET_LIMIT: usize = 2 * 1024 * 1024;
pub const END_LIMIT: u32 = 30 * 86400;
/// Dedicated successful-admission receipt namespace; never consume it on rejection.
pub const ACCEPT_PREFIX: u64 = 0xe000_0000_0000_0000;
pub const JOB_ID_LIMIT: u64 = 0x0fff_ffff_ffff_ffff;
pub fn accept_operation_id(job: u64) -> Result<u64, String> {
    if job == 0 || job > JOB_ID_LIMIT {
        return Err("background_job_id".into());
    }
    Ok(ACCEPT_PREFIX | job)
}
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Failure {
    Transient,
    Cancelled,
    InvalidInput,
}
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case", deny_unknown_fields)]
pub enum Command {
    Enable,
    Restart,
    Lease {
        job: u64,
    },
    /// Atomic execution-epoch fence and lease, including recovery of a running attempt.
    RebindLease {
        job: u64,
    },
    Fail {
        job: u64,
        generation: u32,
        failure: Failure,
    },
    Accept {
        job: u64,
        generation: u32,
        candidate_sha256: [u8; 32],
    },
}
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Job {
    pub id: u64,
    pub base_revision: u64,
    pub field_version: u64,
    pub start: u32,
    pub end: u32,
    pub input: Input,
    pub checkpoint_sha256: [u8; 32],
    pub incarnation: u64,
    pub generation: u32,
    pub attempt: u8,
    pub running: bool,
    pub failure: Option<Failure>,
}
#[derive(Debug, PartialEq, Eq, Serialize)]
pub struct Background {
    field: Resident<State>,
    pending: Resident<Job>,
    version: u64,
    next_job: u64,
    incarnation: u64,
}
/// One computation image, separately owned by the Engine preparation workspace.
/// Not part of the authoritative 320KiB Background limit.
pub(crate) struct Work {
    state: State,
}
impl Work {
    pub(crate) fn backing_ids(&self) -> [usize; 6] {
        self.state.background_allocations()
    }

    pub(crate) fn new() -> Self {
        let work = Self {
            state: State::background_empty(),
        };
        assert!(work.owned_bytes() <= OWNED_LIMIT);
        work
    }
    pub(crate) fn reset(&mut self) {
        self.state.background_reset();
    }
    pub(crate) fn owned_bytes(&self) -> usize {
        self.state.owned_bytes()
    }
}
impl ResidentValue for Job {
    fn empty() -> Self {
        Self {
            id: 0,
            base_revision: 0,
            field_version: 0,
            start: 0,
            end: 0,
            input: Input::background_empty(),
            checkpoint_sha256: [0; 32],
            incarnation: 0,
            generation: 0,
            attempt: 0,
            running: false,
            failure: None,
        }
    }
    fn reset(&mut self) {
        self.input.background_reset();
        self.id = 0;
        self.base_revision = 0;
        self.field_version = 0;
        self.start = 0;
        self.end = 0;
        self.checkpoint_sha256 = [0; 32];
        self.incarnation = 0;
        self.generation = 0;
        self.attempt = 0;
        self.running = false;
        self.failure = None;
    }
}
impl Job {
    fn copy_from(&mut self, v: &Self) -> Result<(), String> {
        self.input.background_copy(&v.input)?;
        self.id = v.id;
        self.base_revision = v.base_revision;
        self.field_version = v.field_version;
        self.start = v.start;
        self.end = v.end;
        self.checkpoint_sha256 = v.checkpoint_sha256;
        self.incarnation = v.incarnation;
        self.generation = v.generation;
        self.attempt = v.attempt;
        self.running = v.running;
        self.failure = v.failure;
        Ok(())
    }
}
impl ResidentValue for Background {
    fn empty() -> Self {
        Self::new()
    }
    fn reset(&mut self) {
        self.field.deactivate();
        self.pending.deactivate();
        self.version = 0;
        self.next_job = 1;
        self.incarnation = 0;
    }
}
impl Clone for Background {
    fn clone(&self) -> Self {
        let mut v = Self::new();
        v.clone_from(self);
        v
    }
    fn clone_from(&mut self, v: &Self) {
        self.copy_from(v).expect("validated bounded background");
    }
}
impl<'de> Deserialize<'de> for Background {
    fn deserialize<D: serde::Deserializer<'de>>(d: D) -> Result<Self, D::Error> {
        #[derive(Deserialize)]
        #[serde(deny_unknown_fields)]
        struct Raw {
            field: Option<State>,
            pending: Option<Job>,
            version: u64,
            next_job: u64,
            incarnation: u64,
        }
        let raw = Raw::deserialize(d)?;
        let mut v = Self::new();
        if let Some(s) = raw.field {
            v.field
                .activate()
                .background_copy(&s)
                .map_err(serde::de::Error::custom)?;
        }
        if let Some(j) = raw.pending {
            v.pending
                .activate()
                .copy_from(&j)
                .map_err(serde::de::Error::custom)?;
        }
        v.version = raw.version;
        v.next_job = raw.next_job;
        v.incarnation = raw.incarnation;
        Ok(v)
    }
}
#[cfg(test)]
fn hash(bytes: &[u8]) -> [u8; 32] {
    use sha2::{Digest, Sha256};
    Sha256::digest(bytes).into()
}
impl Background {
    pub(crate) fn backing_ids(&self) -> [usize; 8] {
        let state = self.field.backing().background_allocations();
        [
            state[0],
            state[1],
            state[2],
            state[3],
            state[4],
            state[5],
            self.pending.backing().input.background_source_allocation(),
            self.pending.backing().input.schema.as_ptr() as usize,
        ]
    }

    pub fn new() -> Self {
        Self {
            field: Resident::empty(),
            pending: Resident::empty(),
            version: 0,
            next_job: 1,
            incarnation: 0,
        }
    }
    fn copy_from(&mut self, v: &Self) -> Result<(), String> {
        match v.field.as_ref() {
            Some(s) => self.field.activate().background_copy(s)?,
            None => self.field.deactivate(),
        }
        match v.pending.as_ref() {
            Some(j) => self.pending.activate().copy_from(j)?,
            None => self.pending.deactivate(),
        }
        self.version = v.version;
        self.next_job = v.next_job;
        self.incarnation = v.incarnation;
        Ok(())
    }
    pub(crate) fn candidate_matches(
        &self,
        candidate: &State,
        work: &mut Work,
        trace: &mut crate::diagnostics::Trace<'_>,
    ) -> Result<bool, String> {
        self.frozen_into(work)?;
        while !work.state.complete() {
            work.state.page_diagnosed(8192, 1024, trace)?;
        }
        Ok(&work.state == candidate)
    }
    fn frozen_into(&self, work: &mut Work) -> Result<(), String> {
        let job = self.pending.as_ref().ok_or("background_no_job")?;
        work.state
            .background_begin(self.field.as_ref(), &job.input)?;
        if work.state.checkpoint_digest()?.1 != job.checkpoint_sha256 {
            return Err("background_input_checksum".into());
        }
        Ok(())
    }
    #[cfg(test)]
    pub(crate) fn allocation_set(&self) -> [usize; 6] {
        self.field.backing().background_allocations()
    }
    pub fn pending(&self) -> Option<&Job> {
        self.pending.as_ref()
    }
    pub fn field(&self) -> Option<&State> {
        self.field.as_ref()
    }
    pub fn version(&self) -> u64 {
        self.version
    }
    pub fn incarnation(&self) -> u64 {
        self.incarnation
    }
    pub fn time(&self) -> u32 {
        self.field.as_ref().map_or(0, |s| s.time())
    }
    fn build_input_state(&self, input: Input) -> Result<State, String> {
        match self.field.as_ref() {
            Some(s) => s.continue_from_boundary(input),
            None => State::new(input),
        }
    }
    /// Same frozen identity, but into caller-owned small-profile backing.
    pub fn frozen_input_into(
        &self,
        slot: &mut crate::air::BackgroundStateSlot,
    ) -> Result<(), String> {
        slot.reset_preserving_capacity();
        let job = self.pending.as_ref().ok_or("background_no_job")?;
        slot.begin_into(self.field.as_ref(), &job.input, job.checkpoint_sha256)
    }
    pub fn frozen_input(&self) -> Result<State, String> {
        let job = self.pending.as_ref().ok_or("background_no_job")?;
        let state = self.build_input_state(job.input.clone())?;
        if state.checkpoint_digest()?.1 != job.checkpoint_sha256 {
            return Err("background_input_checksum".into());
        }
        Ok(state)
    }
    /// P0 only, before clocks and actual dispatch records change. One task per boundary.
    pub(crate) fn capture(
        &mut self,
        exchange: &Exchange,
        start: u64,
        end: u64,
        revision: u64,
        work: &mut Work,
    ) -> Result<(), String> {
        if self.next_job > JOB_ID_LIMIT
            || self.pending.is_some()
            || start != u64::from(self.time())
            || end <= start
            || end > u64::from(END_LIMIT)
        {
            return Err("background_capture_window".into());
        }
        let job = self.pending.activate();
        job.input.target = end as u32;
        for ship in exchange.shipments.iter().flatten() {
            let q = exchange
                .quotes
                .get(ship.quote_index)
                .ok_or("background_quote")?;
            if q.spec.kind != Kind::Freight {
                continue;
            }
            let Some(arrival) = ship.arrives_at else {
                continue;
            };
            let until = arrival.min(end);
            if until <= start {
                continue;
            }
            let map = exchange
                .maps
                .iter()
                .find(|m| m.city == q.spec.from)
                .ok_or("background_map")?;
            // The finite 4096m laboratory field anchors freight at its source warehouse.
            let x = map.origin_x_cm.checked_add(51200).ok_or("background_map")? / 3200;
            if !(0..i64::from(SIDE)).contains(&x) {
                return Err("background_map_extent".into());
            }
            // Experimental EI rate: 1000 microEI / (Q*s), quantity is milliQ.
            let rate_micro = u32::try_from(ship.fragment.quantity.value())
                .map_err(|_| "background_source_rate")?;
            if rate_micro == 0 || rate_micro > 1_000_000 {
                return Err("background_source_rate".into());
            }
            job.input.background_push(Source {
                id: q.spec.id,
                cell: 16 * u32::from(SIDE) + x as u32,
                start: start as u32,
                end: until as u32,
                rate_micro,
            })?;
        }
        job.input.sources.sort_by_key(|s| s.id);
        work.state
            .background_begin(self.field.as_ref(), &job.input)?;
        let (bytes, checkpoint_sha256) = work.state.checkpoint_digest()?;
        if bytes > ASSET_LIMIT {
            return Err("background_asset_bytes".into());
        }
        job.id = self.next_job;
        job.base_revision = revision;
        job.field_version = self.version;
        job.start = start as u32;
        job.end = end as u32;
        job.checkpoint_sha256 = checkpoint_sha256;
        self.next_job = self
            .next_job
            .checked_add(1)
            .ok_or("background_job_overflow")?;
        self.validate_with(work)
    }
    /// No business transfers occur here. The Engine prepares a clone before calling it.
    #[cfg(test)]
    pub(crate) fn command(
        &mut self,
        command: Command,
        candidate: Option<&State>,
    ) -> Result<(), String> {
        self.command_diagnosed(
            command,
            candidate,
            &mut crate::diagnostics::Trace::disabled(),
            &mut Work::new(),
        )
    }
    pub(crate) fn command_diagnosed(
        &mut self,
        command: Command,
        candidate: Option<&State>,
        trace: &mut crate::diagnostics::Trace<'_>,
        work: &mut Work,
    ) -> Result<(), String> {
        match command {
            Command::Enable => return Err("background_duplicate".into()),
            Command::Restart => {
                self.incarnation = self
                    .incarnation
                    .checked_add(1)
                    .ok_or("background_incarnation_overflow")?;
                if let Some(job) = self.pending.as_mut() {
                    job.running = false;
                }
            }
            Command::RebindLease { job: id } => {
                let job = self.pending.as_mut().ok_or("background_no_job")?;
                if job.id != id || job.attempt >= 3 || job.failure == Some(Failure::InvalidInput) {
                    return Err("background_lease".into());
                }
                let incarnation = self
                    .incarnation
                    .checked_add(1)
                    .ok_or("background_incarnation_overflow")?;
                let generation = job
                    .generation
                    .checked_add(1)
                    .ok_or("background_generation_overflow")?;
                self.incarnation = incarnation;
                job.incarnation = incarnation;
                job.generation = generation;
                job.attempt += 1;
                job.running = true;
                job.failure = None;
            }
            Command::Lease { job: id } => {
                let job = self.pending.as_mut().ok_or("background_no_job")?;
                if self.incarnation == 0
                    || job.id != id
                    || job.running
                    || job.attempt >= 3
                    || job.failure == Some(Failure::InvalidInput)
                {
                    return Err("background_lease".into());
                }
                job.incarnation = self.incarnation;
                job.generation = job
                    .generation
                    .checked_add(1)
                    .ok_or("background_generation_overflow")?;
                job.attempt += 1;
                job.running = true;
                job.failure = None;
            }
            Command::Fail {
                job: id,
                generation,
                failure,
            } => {
                let job = self.pending.as_mut().ok_or("background_no_job")?;
                if !job.running
                    || job.id != id
                    || job.generation != generation
                    || job.incarnation != self.incarnation
                {
                    return Err("background_stale_attempt".into());
                }
                job.running = false;
                job.failure = Some(failure);
            }
            Command::Accept {
                job: id,
                generation,
                candidate_sha256,
            } => {
                let job = self.pending.as_ref().ok_or("background_no_job")?;
                if !job.running
                    || job.id != id
                    || job.generation != generation
                    || job.incarnation != self.incarnation
                    || job.field_version != self.version
                {
                    return Err("background_stale_attempt".into());
                }
                let result = candidate.ok_or("background_candidate_missing")?;
                let (bytes, actual_sha256) = result.checkpoint_digest()?;
                if bytes > ASSET_LIMIT
                    || actual_sha256 != candidate_sha256
                    || !result.complete()
                    || result.input() != &job.input
                {
                    return Err("background_candidate_identity".into());
                }
                if !self.candidate_matches(result, work, trace)? {
                    return Err("background_cpu_mismatch".into());
                }
                self.version = self
                    .version
                    .checked_add(1)
                    .ok_or("background_version_overflow")?;
                self.field.activate().background_copy(result)?;
                self.pending.deactivate();
            }
        }
        self.validate_with(work)
    }
    pub fn validate(&self) -> Result<(), String> {
        self.validate_with(&mut Work::new())
    }
    pub(crate) fn validate_with(&self, work: &mut Work) -> Result<(), String> {
        if self.next_job == 0
            || self.next_job > JOB_ID_LIMIT + 1
            || self.version > u64::from(self.time())
            || self.next_job.checked_sub(1)
                != self.version.checked_add(u64::from(self.pending.is_some()))
            || self.field.as_ref().is_some_and(|s| {
                !s.complete()
                    || s.input().side != SIDE
                    || s.input().sources.len() > SOURCE_LIMIT
                    || s.input().initial_micro != 2_000_000
                    || s.input().boundary_micro != [Some(2_000_000); 4]
                    || s.input().boundary_until != END_LIMIT
            })
            || (self.version == 0) == self.field.is_some()
            || self.owned_bytes() > OWNED_LIMIT
        {
            return Err("background_state".into());
        }
        if let Some(job) = self.pending.as_ref() {
            if job.id == 0
                || job.id.checked_add(1) != Some(self.next_job)
                || job.base_revision == 0
                || job.field_version != self.version
                || job.start != self.time()
                || job.end <= job.start
                || job.end != job.input.target
                || job.input.side != SIDE
                || job.input.initial_micro != 2_000_000
                || job.input.boundary_micro != [Some(2_000_000); 4]
                || job.input.boundary_until != END_LIMIT
                || job.input.sources.len() > SOURCE_LIMIT
                || job.input.sources.iter().any(|s| s.start < job.start)
                || job.attempt > 3
                || job.generation != u32::from(job.attempt)
                || job.incarnation > self.incarnation
                || (job.running
                    && (job.attempt == 0
                        || job.incarnation != self.incarnation
                        || job.failure.is_some()))
            {
                return Err("background_job_state".into());
            }
            self.frozen_into(work)?;
        }
        Ok(())
    }
    pub(crate) fn validate_timeline(
        &self,
        timeline: &crate::timeline::Timeline,
        revision: u64,
    ) -> Result<(), String> {
        use crate::schedule::Phase;
        let p = timeline.progress;
        let time = u64::from(self.time());
        let valid = if p.at == p.last_complete {
            !self.pending.is_some() && time == p.at
        } else {
            match p.phase {
                None | Some(Phase::P0) => !self.pending.is_some() && time == p.last_complete,
                Some(Phase::P1 | Phase::P2 | Phase::P3) => {
                    self.pending.is_some() && time == p.last_complete
                }
                Some(Phase::P4) => {
                    if self.pending.is_some() {
                        time == p.last_complete
                    } else {
                        time == p.at
                    }
                }
                Some(Phase::P5) => !self.pending.is_some() && time == p.at,
            }
        };
        if !valid
            || p.at > u64::from(END_LIMIT)
            || self.pending.as_ref().is_some_and(|j| {
                j.base_revision > revision
                    || u64::from(j.start) != p.last_complete
                    || u64::from(j.end) != p.at
            })
        {
            return Err("background_timeline_state".into());
        }
        Ok(())
    }
    pub fn owned_bytes(&self) -> usize {
        size_of::<Self>() + self.field.backing().owned_bytes() - size_of::<State>()
            + self.pending.backing().input.schema.capacity()
            + self.pending.backing().input.sources.capacity() * size_of::<Source>()
    }
    pub fn state_hash(&self) -> String {
        sha256(&serde_json::to_vec(self).expect("bounded background state"))
    }
}
impl Default for Background {
    fn default() -> Self {
        Self::new()
    }
}

impl crate::restore_into::Restore for Background {
    fn restore<'de, D: serde::Deserializer<'de>>(&mut self, d: D) -> Result<(), D::Error> {
        crate::restore_into::object!(d,self,Background,28u64,{
        field,0=>Into,
        pending,1=>Into,
        version,2=>Copy,
        next_job,3=>Copy,
        incarnation,4=>Copy,
        })?;
        Ok(())
    }
}

impl crate::restore_into::Restore for Job {
    fn restore<'de, D: serde::Deserializer<'de>>(&mut self, d: D) -> Result<(), D::Error> {
        crate::restore_into::object!(d,self,Job,2047u64,{
        id,0=>Copy,
        base_revision,1=>Copy,
        field_version,2=>Copy,
        start,3=>Copy,
        end,4=>Copy,
        input,5=>Into,
        checkpoint_sha256,6=>Copy,
        incarnation,7=>Copy,
        generation,8=>Copy,
        attempt,9=>Copy,
        running,10=>Copy,
        failure,11=>Copy,
        })?;
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{
        assembly::assemble,
        exchange::Command as X,
        transaction::{Action, Caller, Engine, Envelope, Resource},
    };
    fn engine() -> Engine {
        Engine::new(
            assemble(include_bytes!("../../../fixtures/m0-exchange-opening.json")).unwrap(),
            [
                (101, 1),
                (102, 1),
                (103, 1),
                (104, 1),
                (201, 2000),
                (202, 2000),
            ]
            .into_iter()
            .map(|(id, cap)| Resource::new(id, cap).unwrap())
            .collect(),
        )
        .unwrap()
    }
    fn run(e: &mut Engine, command: X) {
        let revision = e.world().view().revision;
        e.execute(
            Caller::System,
            Envelope {
                world: 1,
                branch: 1,
                operation_id: revision + 1,
                expected_revision: revision,
                actions: vec![Action::Exchange { command }],
            },
        )
        .unwrap();
    }
    fn finish(b: &Background) -> State {
        let mut state = b.frozen_input().unwrap();
        while !state.complete() {
            state.page(8192, 1024).unwrap();
        }
        state
    }
    #[test]
    fn real_dispatched_freight_produces_bounded_source_but_paused_link_does_not() {
        let mut e = engine();
        let initial = e.world().view().exchange.unwrap();
        let mut empty = Background::new();
        empty.capture(&initial, 0, 60, 1, &mut Work::new()).unwrap();
        assert!(empty.pending().unwrap().input.sources.is_empty());
        run(&mut e, X::Dispatch { quote: 1 });
        let exchange = e.world().view().exchange.unwrap();
        let mut b = Background::new();
        b.capture(&exchange, 0, 60, 2, &mut Work::new()).unwrap();
        let source = &b.pending().unwrap().input.sources[0];
        assert_eq!(source.cell, 16 * 128 + 16);
        assert_eq!(
            source.rate_micro as i64,
            exchange
                .shipments
                .iter()
                .flatten()
                .next()
                .unwrap()
                .fragment
                .quantity
                .value()
        );
        run(
            &mut e,
            X::SetLink {
                edge: exchange.quotes[0].spec.edges[0].unwrap(),
                open: false,
            },
        );
        let mut paused = Background::new();
        paused
            .capture(
                &e.world().view().exchange.unwrap(),
                0,
                60,
                3,
                &mut Work::new(),
            )
            .unwrap();
        assert!(paused.pending().unwrap().input.sources.is_empty());
    }
    #[test]
    fn restart_fences_old_attempt_and_preserves_continuous_committed_field() {
        let mut e = engine();
        run(&mut e, X::Dispatch { quote: 1 });
        let exchange = e.world().view().exchange.unwrap();
        let mut b = Background::new();
        b.capture(&exchange, 0, 60, 2, &mut Work::new()).unwrap();
        b.command(Command::Restart, None).unwrap();
        b.command(Command::Lease { job: 1 }, None).unwrap();
        let result = finish(&b);
        let digest = hash(&result.checkpoint().unwrap());
        b.command(Command::Restart, None).unwrap();
        assert!(b
            .command(
                Command::Accept {
                    job: 1,
                    generation: 1,
                    candidate_sha256: digest
                },
                Some(&result)
            )
            .is_err());
        b.command(Command::Lease { job: 1 }, None).unwrap();
        assert!(b
            .command(
                Command::Accept {
                    job: 1,
                    generation: 1,
                    candidate_sha256: digest
                },
                Some(&result)
            )
            .is_err());
        b.command(
            Command::Accept {
                job: 1,
                generation: 2,
                candidate_sha256: digest,
            },
            Some(&result),
        )
        .unwrap();
        assert_eq!(b.field(), Some(&result));
        assert!(b.pending().is_none());
        assert_eq!(b.version(), 1);
        let mut restored: Background =
            serde_json::from_slice(&serde_json::to_vec(&b).unwrap()).unwrap();
        restored.validate().unwrap();
        restored
            .capture(&exchange, 60, 120, 10, &mut Work::new())
            .unwrap();
        assert_eq!(
            restored.frozen_input().unwrap().field_bits(),
            result.field_bits()
        );
        assert_eq!(
            restored.frozen_input().unwrap().integral_bits(),
            result.integral_bits()
        );
        assert!(restored.owned_bytes() <= OWNED_LIMIT);
    }
    #[test]
    fn three_failed_attempts_or_bad_input_leave_required_job_without_unbounded_retry() {
        let mut e = engine();
        run(&mut e, X::Dispatch { quote: 1 });
        let exchange = e.world().view().exchange.unwrap();
        let mut b = Background::new();
        b.capture(&exchange, 0, 60, 2, &mut Work::new()).unwrap();
        b.command(Command::Restart, None).unwrap();
        for generation in 1..=3 {
            b.command(Command::Lease { job: 1 }, None).unwrap();
            b.command(
                Command::Fail {
                    job: 1,
                    generation,
                    failure: Failure::Transient,
                },
                None,
            )
            .unwrap();
        }
        let before = b.state_hash();
        assert!(b.command(Command::Lease { job: 1 }, None).is_err());
        assert_eq!(before, b.state_hash());
        b.command(Command::Restart, None).unwrap();
        assert!(b.command(Command::Lease { job: 1 }, None).is_err());
        assert!(b.pending().is_some());
        assert_eq!(b.time(), 0);
        let mut b = Background::new();
        b.capture(&exchange, 0, 60, 2, &mut Work::new()).unwrap();
        b.command(Command::Restart, None).unwrap();
        b.command(Command::Lease { job: 1 }, None).unwrap();
        b.command(
            Command::Fail {
                job: 1,
                generation: 1,
                failure: Failure::InvalidInput,
            },
            None,
        )
        .unwrap();
        assert!(b.command(Command::Lease { job: 1 }, None).is_err());
        assert_eq!(b.pending().unwrap().attempt, 1);
    }
    #[test]
    fn fixed_background_lifecycle_keeps_all_backings_and_old_numerical_bytes() {
        let mut e = engine();
        run(&mut e, X::Dispatch { quote: 1 });
        let exchange = e.world().view().exchange.unwrap();
        let mut b = Background::new();
        let mut work = Work::new();
        let field_ids = b.field.backing().background_allocations();
        let work_ids = work.state.background_allocations();
        let pending_sources = b.pending.backing().input.sources.as_ptr();
        let mut mirror = Background::new();
        let mirror_ids = mirror.field.backing().background_allocations();
        for (i, (start, end)) in [(0, 60), (60, 120)].into_iter().enumerate() {
            b.capture(&exchange, start, end, 2 + i as u64, &mut work)
                .unwrap();
            b.command_diagnosed(
                Command::RebindLease { job: 1 + i as u64 },
                None,
                &mut crate::diagnostics::Trace::disabled(),
                &mut work,
            )
            .unwrap();
            let legacy = b.frozen_input().unwrap();
            let mut slot = crate::air::BackgroundStateSlot::new();
            let slot_id = slot.identity();
            b.frozen_input_into(&mut slot).unwrap();
            assert_eq!(
                slot.state().unwrap().checkpoint().unwrap(),
                legacy.checkpoint().unwrap()
            );
            b.pending.as_mut().unwrap().checkpoint_sha256[0] ^= 1;
            assert!(b.frozen_input_into(&mut slot).is_err());
            assert!(slot.state().is_err());
            assert_eq!(slot.identity(), slot_id);
            b.pending.as_mut().unwrap().checkpoint_sha256[0] ^= 1;
            b.frozen_input_into(&mut slot).unwrap();
            assert!(Background::new().frozen_input_into(&mut slot).is_err());
            assert!(slot.state().is_err());
            assert_eq!(slot.identity(), slot_id);
            b.frozen_into(&mut work).unwrap();
            assert_eq!(
                legacy.checkpoint().unwrap(),
                work.state.checkpoint().unwrap()
            );
            let result = finish(&b);
            let digest = result.checkpoint_digest().unwrap().1;
            // Wrong hash fails without changing authority; scratch can be reused.
            let before = b.state_hash();
            assert!(b
                .command_diagnosed(
                    Command::Accept {
                        job: 1 + i as u64,
                        generation: 1,
                        candidate_sha256: [0; 32]
                    },
                    Some(&result),
                    &mut crate::diagnostics::Trace::disabled(),
                    &mut work
                )
                .is_err());
            assert_eq!(b.state_hash(), before);
            b.command_diagnosed(
                Command::Accept {
                    job: 1 + i as u64,
                    generation: 1,
                    candidate_sha256: digest,
                },
                Some(&result),
                &mut crate::diagnostics::Trace::disabled(),
                &mut work,
            )
            .unwrap();
            assert_eq!(
                b.field().unwrap().checkpoint().unwrap(),
                result.checkpoint().unwrap()
            );
            mirror.clone_from(&b);
            assert_eq!(
                serde_json::to_vec(&mirror).unwrap(),
                serde_json::to_vec(&b).unwrap()
            );
            assert_eq!(mirror.field.backing().background_allocations(), mirror_ids);
            assert_eq!(b.field.backing().background_allocations(), field_ids);
            assert_eq!(b.pending.backing().input.sources.as_ptr(), pending_sources);
            assert_eq!(work.state.background_allocations(), work_ids);
            assert!(b.owned_bytes() <= OWNED_LIMIT);
        }
        b.reset();
        work.reset();
        mirror.reset();
        assert_eq!(b.field.backing().background_allocations(), field_ids);
        assert_eq!(work.state.background_allocations(), work_ids);
        assert_eq!(
            serde_json::to_string(&b).unwrap(),
            r#"{"field":null,"pending":null,"version":0,"next_job":1,"incarnation":0}"#
        );
    }
}
