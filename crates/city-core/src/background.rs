//! Finite regional air obligation. Sim owns this state; workers only propose a candidate.
use crate::{
    air::{Input, Source, State, STRATEGY},
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
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Background {
    field: Option<State>,
    pending: Option<Job>,
    version: u64,
    next_job: u64,
    incarnation: u64,
}
fn hash(bytes: &[u8]) -> [u8; 32] {
    use sha2::{Digest, Sha256};
    Sha256::digest(bytes).into()
}
impl Background {
    pub fn new() -> Self {
        Self {
            field: None,
            pending: None,
            version: 0,
            next_job: 1,
            incarnation: 0,
        }
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
        match &self.field {
            Some(s) => s.continue_from_boundary(input),
            None => State::new(input),
        }
    }
    pub fn frozen_input(&self) -> Result<State, String> {
        let job = self.pending.as_ref().ok_or("background_no_job")?;
        let state = self.build_input_state(job.input.clone())?;
        if hash(&state.checkpoint()?) != job.checkpoint_sha256 {
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
    ) -> Result<(), String> {
        if self.pending.is_some()
            || start != u64::from(self.time())
            || end <= start
            || end > u64::from(END_LIMIT)
        {
            return Err("background_capture_window".into());
        }
        let mut sources = Vec::new();
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
            sources.push(Source {
                id: q.spec.id,
                cell: 16 * u32::from(SIDE) + x as u32,
                start: start as u32,
                end: until as u32,
                rate_micro,
            });
        }
        sources.sort_by_key(|s| s.id);
        if sources.len() > SOURCE_LIMIT {
            return Err("background_source_limit".into());
        }
        let input = Input {
            schema: STRATEGY.into(),
            side: SIDE,
            target: end as u32,
            initial_micro: 2_000_000,
            boundary_micro: [Some(2_000_000); 4],
            boundary_until: END_LIMIT,
            sources,
        };
        let state = self.build_input_state(input.clone())?;
        let bytes = state.checkpoint()?;
        if bytes.len() > ASSET_LIMIT {
            return Err("background_asset_bytes".into());
        }
        self.pending = Some(Job {
            id: self.next_job,
            base_revision: revision,
            field_version: self.version,
            start: start as u32,
            end: end as u32,
            input,
            checkpoint_sha256: hash(&bytes),
            incarnation: 0,
            generation: 0,
            attempt: 0,
            running: false,
            failure: None,
        });
        self.next_job = self
            .next_job
            .checked_add(1)
            .ok_or("background_job_overflow")?;
        self.validate()
    }
    /// No business transfers occur here. The Engine prepares a clone before calling it.
    pub(crate) fn command(
        &mut self,
        command: Command,
        candidate: Option<&State>,
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
                let bytes = result.checkpoint()?;
                if bytes.len() > ASSET_LIMIT
                    || hash(&bytes) != candidate_sha256
                    || !result.complete()
                    || result.input() != &job.input
                {
                    return Err("background_candidate_identity".into());
                }
                let mut reference = self.frozen_input()?;
                while !reference.complete() {
                    reference.page(8192, 1024)?;
                }
                if &reference != result {
                    return Err("background_cpu_mismatch".into());
                }
                self.version = self
                    .version
                    .checked_add(1)
                    .ok_or("background_version_overflow")?;
                self.field = Some(result.clone());
                self.pending = None;
            }
        }
        self.validate()
    }
    pub fn validate(&self) -> Result<(), String> {
        if self.next_job == 0
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
            || (self.version == 0) != self.field.is_none()
            || self.owned_bytes() > OWNED_LIMIT
        {
            return Err("background_state".into());
        }
        if let Some(job) = &self.pending {
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
            self.frozen_input()?;
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
            self.pending.is_none() && time == p.at
        } else {
            match p.phase {
                None | Some(Phase::P0) => self.pending.is_none() && time == p.last_complete,
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
                Some(Phase::P5) => self.pending.is_none() && time == p.at,
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
        size_of::<Self>()
            + self
                .field
                .as_ref()
                .map_or(0, |s| s.owned_bytes() - size_of::<State>())
            + self.pending.as_ref().map_or(0, |j| {
                j.input.schema.capacity() + j.input.sources.capacity() * size_of::<Source>()
            })
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
        empty.capture(&initial, 0, 60, 1).unwrap();
        assert!(empty.pending().unwrap().input.sources.is_empty());
        run(&mut e, X::Dispatch { quote: 1 });
        let exchange = e.world().view().exchange.unwrap();
        let mut b = Background::new();
        b.capture(&exchange, 0, 60, 2).unwrap();
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
            .capture(&e.world().view().exchange.unwrap(), 0, 60, 3)
            .unwrap();
        assert!(paused.pending().unwrap().input.sources.is_empty());
    }
    #[test]
    fn restart_fences_old_attempt_and_preserves_continuous_committed_field() {
        let mut e = engine();
        run(&mut e, X::Dispatch { quote: 1 });
        let exchange = e.world().view().exchange.unwrap();
        let mut b = Background::new();
        b.capture(&exchange, 0, 60, 2).unwrap();
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
        restored.capture(&exchange, 60, 120, 10).unwrap();
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
        b.capture(&exchange, 0, 60, 2).unwrap();
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
        b.capture(&exchange, 0, 60, 2).unwrap();
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
}
