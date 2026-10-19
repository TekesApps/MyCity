//! Bounded static due-source scheduler. Page acknowledgement is volatile until storage integration.
use crate::{arena::Scope, sha256, units::GameTime};
use serde::{Deserialize, Serialize};
use std::{
    mem::size_of,
    sync::atomic::{AtomicU64, Ordering},
};

pub const READY_LIMIT: usize = 8192;
pub const SOURCE_LIMIT: usize = 65536;
pub const MAX_INTERVAL_SECONDS: i64 = 3600;
static NEXT_INSTANCE: AtomicU64 = AtomicU64::new(1);

#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
pub enum Phase {
    P0,
    P1,
    P2,
    P3,
    P4,
    P5,
}
impl Phase {
    fn next(self) -> Option<Self> {
        match self {
            Self::P0 => Some(Self::P1),
            Self::P1 => Some(Self::P2),
            Self::P2 => Some(Self::P3),
            Self::P3 => Some(Self::P4),
            Self::P4 => Some(Self::P5),
            Self::P5 => None,
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Due {
    pub at: GameTime,
    pub phase: Phase,
    pub source: u64,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Bookmark {
    pub scope: Scope,
    pub schema: String,
    pub source_sha256: String,
    pub at: GameTime,
    pub last_complete: Option<GameTime>,
    pub phase: Option<Phase>,
    pub cursor: usize,
    pub sequence: u64,
    pub paused: bool,
    pub interval_closed: bool,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Ticket {
    instance: u64,
    sequence: u64,
    start: usize,
    end: usize,
    at: GameTime,
    phase: Phase,
}
pub struct Page<'a> {
    pub ticket: Ticket,
    pub events: &'a [Due],
    pub close_interval: Option<(GameTime, GameTime)>,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ScheduleError {
    Limit,
    DuplicateSource,
    Overflow,
    InvalidBookmark,
    StaleTicket,
    Backwards,
    Allocation,
}

pub struct Schedule {
    sources: Vec<Due>,
    book: Bookmark,
    instance: u64,
}

impl Schedule {
    pub fn new(scope: Scope, mut sources: Vec<Due>) -> Result<Self, ScheduleError> {
        if sources.len() > SOURCE_LIMIT || sources.capacity() > SOURCE_LIMIT {
            return Err(ScheduleError::Limit);
        }
        sources.sort_by_key(|s| (s.at, s.phase, s.source));
        // Source ids identify one immutable scheduled occurrence, not one recurring owner.
        let mut ids = Vec::new();
        ids.try_reserve_exact(sources.len())
            .map_err(|_| ScheduleError::Allocation)?;
        ids.extend(sources.iter().map(|s| s.source));
        ids.sort_unstable();
        if ids.first() == Some(&0) || ids.windows(2).any(|p| p[0] == p[1]) {
            return Err(ScheduleError::DuplicateSource);
        }
        let hash = sha256(&serde_json::to_vec(&sources).map_err(|_| ScheduleError::Limit)?);
        let instance = NEXT_INSTANCE
            .fetch_update(Ordering::Relaxed, Ordering::Relaxed, |n| n.checked_add(1))
            .map_err(|_| ScheduleError::Overflow)?;
        Ok(Self {
            sources,
            instance,
            book: Bookmark {
                scope,
                schema: "mycity-m0-schedule-v1".into(),
                source_sha256: hash,
                at: GameTime::ZERO,
                last_complete: None,
                phase: Some(Phase::P0),
                cursor: 0,
                sequence: 0,
                paused: false,
                interval_closed: false,
            },
        })
    }
    pub fn bookmark(&self) -> Bookmark {
        self.book.clone()
    }
    pub fn set_paused(&mut self, paused: bool) {
        self.book.paused = paused;
    }
    pub fn owned_source_capacity_bytes(&self) -> usize {
        self.sources.capacity() * size_of::<Due>()
    }
    pub fn restore(scope: Scope, sources: Vec<Due>, book: Bookmark) -> Result<Self, ScheduleError> {
        let mut schedule = Self::new(scope, sources)?;
        if book.scope != scope
            || book.schema != schedule.book.schema
            || book.source_sha256 != schedule.book.source_sha256
            || book.cursor > schedule.sources.len()
            || book.last_complete.is_some_and(|t| t > book.at)
        {
            return Err(ScheduleError::InvalidBookmark);
        }
        if book.phase.is_none() && (book.last_complete != Some(book.at) || !book.interval_closed) {
            return Err(ScheduleError::InvalidBookmark);
        }
        if book.phase.is_some() && book.last_complete.is_some_and(|t| t >= book.at) {
            return Err(ScheduleError::InvalidBookmark);
        }
        if book.phase.is_some_and(|p| p != Phase::P0) && !book.interval_closed {
            return Err(ScheduleError::InvalidBookmark);
        }
        if book.phase.is_some()
            && book.at.value() - book.last_complete.unwrap_or(GameTime::ZERO).value()
                > MAX_INTERVAL_SECONDS
        {
            return Err(ScheduleError::InvalidBookmark);
        }
        for (i, event) in schedule.sources.iter().enumerate() {
            let before = event.at < book.at
                || (event.at == book.at && book.phase.is_none_or(|p| event.phase < p));
            let after = event.at > book.at
                || (event.at == book.at && book.phase.is_some_and(|p| event.phase > p));
            if (before && i >= book.cursor) || (after && i < book.cursor) {
                return Err(ScheduleError::InvalidBookmark);
            }
        }
        schedule.book = book;
        Ok(schedule)
    }
    pub fn next_page(&mut self, target: GameTime) -> Result<Option<Page<'_>>, ScheduleError> {
        if self.book.paused {
            return Ok(None);
        }
        if target < self.book.at {
            return Err(ScheduleError::Backwards);
        }
        if self.book.phase.is_none() {
            if target == self.book.at {
                return Ok(None);
            }
            let delta = (target.value() - self.book.at.value()).min(MAX_INTERVAL_SECONDS);
            let cap = self
                .book
                .at
                .checked_add(GameTime::new(delta).map_err(|_| ScheduleError::Overflow)?)
                .map_err(|_| ScheduleError::Overflow)?;
            let next_event = self
                .sources
                .get(self.book.cursor)
                .map(|e| e.at)
                .unwrap_or(target);
            self.book.at = target.min(cap).min(next_event);
            self.book.phase = Some(Phase::P0);
            self.book.interval_closed = false;
        }
        let phase = self.book.phase.ok_or(ScheduleError::InvalidBookmark)?;
        let start = self.book.cursor;
        let mut end = start;
        while end < self.sources.len()
            && end - start < READY_LIMIT
            && self.sources[end].at == self.book.at
            && self.sources[end].phase == phase
        {
            end += 1;
        }
        let ticket = Ticket {
            instance: self.instance,
            sequence: self.book.sequence,
            start,
            end,
            at: self.book.at,
            phase,
        };
        let close_interval = if phase == Phase::P0 && !self.book.interval_closed {
            Some((
                self.book.last_complete.unwrap_or(GameTime::ZERO),
                self.book.at,
            ))
        } else {
            None
        };
        Ok(Some(Page {
            ticket,
            events: &self.sources[start..end],
            close_interval,
        }))
    }
    pub fn acknowledge(&mut self, ticket: Ticket) -> Result<(), ScheduleError> {
        if self.book.paused
            || ticket.instance != self.instance
            || ticket.sequence != self.book.sequence
            || ticket.start != self.book.cursor
            || ticket.at != self.book.at
            || Some(ticket.phase) != self.book.phase
        {
            return Err(ScheduleError::StaleTicket);
        }
        let sequence = self
            .book
            .sequence
            .checked_add(1)
            .ok_or(ScheduleError::Overflow)?;
        if ticket.phase == Phase::P0 {
            self.book.interval_closed = true;
        }
        self.book.cursor = ticket.end;
        self.book.sequence = sequence;
        if !self
            .sources
            .get(ticket.end)
            .is_some_and(|e| e.at == ticket.at && e.phase == ticket.phase)
        {
            self.book.phase = ticket.phase.next();
            if self.book.phase.is_none() {
                self.book.last_complete = Some(self.book.at);
            }
        }
        Ok(())
    }
}

/// Stable per-source randomness; independent of pages, threads, UI, and wall clock.
pub fn random_u64(seed: u64, domain: u16, source: u64, window: u64) -> u64 {
    use sha2::{Digest, Sha256};
    let mut h = Sha256::new();
    h.update(b"mycity-source-rng-v1");
    h.update(seed.to_le_bytes());
    h.update(domain.to_le_bytes());
    h.update(source.to_le_bytes());
    h.update(window.to_le_bytes());
    u64::from_le_bytes(h.finalize()[..8].try_into().expect("SHA256 length"))
}

#[cfg(test)]
mod tests {
    use super::*;
    fn time(n: i64) -> GameTime {
        GameTime::new(n).unwrap()
    }
    fn sources() -> Vec<Due> {
        (1..=20000)
            .map(|source| Due {
                at: time(3600),
                phase: Phase::P2,
                source,
            })
            .collect()
    }
    fn finish(s: &mut Schedule, target: GameTime) -> Vec<u64> {
        let mut ids = vec![];
        while let Some(p) = s.next_page(target).unwrap() {
            ids.extend(p.events.iter().map(|e| e.source));
            let t = p.ticket;
            s.acknowledge(t).unwrap();
        }
        ids
    }
    #[test]
    fn large_same_time_source_pages_finish_before_advancing_world() {
        let mut s = Schedule::new(Scope::new(1, 1).unwrap(), sources()).unwrap();
        let mut count = 0;
        let mut max = 0;
        while let Some(p) = s.next_page(time(3601)).unwrap() {
            max = max.max(p.events.len());
            count += p.events.len();
            let t = p.ticket;
            if !p.events.is_empty() {
                assert_eq!(s.book.last_complete, Some(time(0)));
            }
            s.acknowledge(t).unwrap();
        }
        assert_eq!(count, 20000);
        assert_eq!(max, 8192);
        assert_eq!(s.book.last_complete, Some(time(3601)));
    }
    #[test]
    fn bookmark_continues_remaining_pages_without_reprocessing_old_sources() {
        let mut s = Schedule::new(Scope::new(1, 1).unwrap(), sources()).unwrap();
        let mut before = vec![];
        loop {
            let p = s.next_page(time(3601)).unwrap().unwrap();
            before.extend(p.events.iter().map(|e| e.source));
            let t = p.ticket;
            s.acknowledge(t).unwrap();
            if before.len() == 8192 {
                break;
            }
        }
        let old = s.next_page(time(3601)).unwrap().unwrap().ticket;
        let book: Bookmark =
            serde_json::from_slice(&serde_json::to_vec(&s.bookmark()).unwrap()).unwrap();
        let mut restored = Schedule::restore(Scope::new(1, 1).unwrap(), sources(), book).unwrap();
        assert_eq!(restored.acknowledge(old), Err(ScheduleError::StaleTicket));
        before.extend(finish(&mut restored, time(3601)));
        assert_eq!(before, (1..=20000).collect::<Vec<_>>());
    }
    #[test]
    fn pause_and_old_ack_do_not_change_cursor_or_time() {
        let mut s = Schedule::new(Scope::new(1, 1).unwrap(), vec![]).unwrap();
        let t = s.next_page(time(10)).unwrap().unwrap().ticket;
        s.set_paused(true);
        let book = s.bookmark();
        assert!(s.next_page(time(999)).unwrap().is_none());
        assert_eq!(s.acknowledge(t), Err(ScheduleError::StaleTicket));
        assert_eq!(s.bookmark(), book);
        s.set_paused(false);
        s.acknowledge(t).unwrap();
        assert_eq!(s.acknowledge(t), Err(ScheduleError::StaleTicket));
    }
    #[test]
    fn phase_order_and_interval_cap_are_independent_of_input_order() {
        let mut events = vec![
            Due {
                at: time(4000),
                phase: Phase::P5,
                source: 1,
            },
            Due {
                at: time(4000),
                phase: Phase::P1,
                source: 2,
            },
            Due {
                at: time(4000),
                phase: Phase::P0,
                source: 3,
            },
        ];
        events.reverse();
        let mut s = Schedule::new(Scope::new(1, 1).unwrap(), events).unwrap();
        let mut actual = vec![];
        let mut intervals = vec![];
        while let Some(p) = s.next_page(time(7200)).unwrap() {
            if let Some((a, b)) = p.close_interval {
                assert!(b.value() - a.value() <= 3600);
                intervals.push((a, b));
            }
            actual.extend(p.events.iter().map(|e| e.source));
            let t = p.ticket;
            s.acknowledge(t).unwrap();
        }
        assert_eq!(actual, vec![3, 2, 1]);
        assert!(intervals.iter().any(|(_, b)| *b == time(4000)));
    }
    #[test]
    fn malformed_bookmarks_and_changed_sources_reject() {
        let mut s = Schedule::new(Scope::new(1, 1).unwrap(), sources()).unwrap();
        finish(&mut s, time(3601));
        let book = s.bookmark();
        let mut bad = book.clone();
        bad.cursor = 0;
        assert!(Schedule::restore(Scope::new(1, 1).unwrap(), sources(), bad).is_err());
        let mut bad = book.clone();
        bad.cursor = usize::MAX;
        assert!(Schedule::restore(Scope::new(1, 1).unwrap(), sources(), bad).is_err());
        assert!(Schedule::restore(Scope::new(1, 1).unwrap(), vec![], book).is_err());
        assert!(Schedule::new(
            Scope::new(1, 1).unwrap(),
            vec![
                Due {
                    at: time(0),
                    phase: Phase::P0,
                    source: 1
                };
                2
            ]
        )
        .is_err());
    }
    #[test]
    fn per_source_randomness_ignores_paging_and_is_domain_separated() {
        let a: Vec<_> = (1..=20000).map(|id| random_u64(7, 1, id, 30)).collect();
        let b: Vec<_> = (1..=20000)
            .collect::<Vec<_>>()
            .chunks(8192)
            .flat_map(|page| page.iter().map(|id| random_u64(7, 1, *id, 30)))
            .collect();
        assert_eq!(a, b);
        assert_ne!(random_u64(7, 1, 1, 30), random_u64(7, 2, 1, 30));
    }

    #[test]
    fn bookmark_cannot_be_loaded_into_another_world_or_branch() {
        let s = Schedule::new(Scope::new(1, 1).unwrap(), sources()).unwrap();
        for scope in [Scope::new(2, 1).unwrap(), Scope::new(1, 2).unwrap()] {
            assert!(Schedule::restore(scope, sources(), s.bookmark()).is_err());
        }
        assert!(serde_json::from_str::<Scope>(r#"{"world":0,"branch":1}"#).is_err());
    }
}
