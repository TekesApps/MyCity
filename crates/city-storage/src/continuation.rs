//! A finite normal-funded reservation for one already accepted timeline boundary.
use super::*;
use mycity_core::{timeline, transaction::Action};
pub const CONTINUATION_PAGE_BYTES: u64 = FRAME_LIMIT as u64 + 64 + 3 * META_LIMIT;
pub const CONTINUATION_MAX_PAGES: u32 = 96;
const CONTROL_SLOTS: usize = 64;
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Continuation {
    pub start: u64,
    pub at: u64,
    pub target: u64,
    pub next_serial: u64,
    pub page_limit: u16,
    pub remaining_pages: u32,
    pub remaining_bytes: u64,
}
impl Continuation {
    pub(super) fn valid(&self) -> bool {
        self.at >= self.start
            && self.at - self.start <= 3600
            && self.target >= self.at
            && self.page_limit > 0
            && self.page_limit <= timeline::PAGE_LIMIT
            && self.remaining_pages > 0
            && self.remaining_pages <= CONTINUATION_MAX_PAGES
            && self.remaining_bytes == u64::from(self.remaining_pages) * CONTINUATION_PAGE_BYTES
    }
}
impl Store {
    pub fn continuation_status(&self) -> Result<Option<Continuation>, Error> {
        self.engine()?;
        Ok(self.io.root.continuation.clone())
    }
    pub(super) fn validate_continuation(engine: &Engine, root: &Root) -> Result<(), Error> {
        match (&root.continuation, engine.timeline()) {
            (Some(c), Some(t))
                if c.valid()
                    && t.progress.phase.is_some()
                    && c.start == t.progress.last_complete
                    && c.at == t.progress.at
                    && Some(c.target) == t.progress.target
                    && c.next_serial == t.progress.serial =>
            {
                Ok(())
            }
            (None, t) if t.is_none_or(|t| t.progress.phase.is_none()) => Ok(()),
            _ => Err(Error::Invalid("timeline_continuation_mismatch")),
        }
    }
    pub(super) fn prepare_continuation(
        &mut self,
        caller: Caller,
        env: &Envelope,
        background_funded: bool,
    ) -> Result<Option<Continuation>, Error> {
        let limit = match env.actions.as_slice() {
            [Action::Timeline {
                command: timeline::Command::Advance { limit, .. },
            }] => Some(*limit),
            _ => None,
        };
        if let Some(c) = &self.io.root.continuation {
            if let Some(limit) = limit {
                if limit != c.page_limit {
                    return Err(Error::Invalid("timeline_reserved_page_limit"));
                }
                if self
                    .engine
                    .timeline_quote(caller, env)
                    .is_some_and(|(p, _)| {
                        p.at == c.at
                            && p.serial == c.next_serial + 1
                            && (p.target == Some(c.target) || p.phase.is_none())
                    })
                {
                    return Ok(Some(c.clone()));
                }
            }
            // Keep sufficient receipt slots for the clock, plus a separate finite reserve for
            // real source/cleanup progress. Failed or unrelated requests cannot spend that reserve.
            let eligible =
                background_funded || self.engine.can_advance_existing_responsibility(caller, env);
            let floor = c.remaining_pages as usize + if eligible { 0 } else { CONTROL_SLOTS };
            if self.engine.journal().len() + 1 + floor > MAX_RECEIPTS {
                return Err(Error::Invalid("timeline_receipt_reserve"));
            }
            return Ok(None);
        }
        let Some(limit) = limit else { return Ok(None) };
        let Some((progress, steps)) = self.engine.timeline_quote(caller, env) else {
            return Ok(None);
        };
        // P4 forces one extra page boundary independent of the requested page width.
        let barrier = usize::from(
            self.engine.background().is_some()
                && progress.at
                    > self
                        .engine
                        .timeline()
                        .expect("quoted timeline")
                        .progress
                        .last_complete,
        );
        let pages = steps.div_ceil(usize::from(limit)) + barrier;
        if pages == 0 || pages > CONTINUATION_MAX_PAGES as usize {
            return Err(Error::Invalid("timeline_page_limit_too_small"));
        }
        // Pre-admit the whole boundary; finishing never depends on a later checkpoint.
        if self.engine.journal().len() + pages + CONTROL_SLOTS > MAX_RECEIPTS
            || self.io.root.wal_end + (pages as u64) * CONTINUATION_PAGE_BYTES > WAL_LIMIT
        {
            self.checkpoint()?;
        }
        let c = Continuation {
            start: self
                .engine
                .timeline()
                .ok_or(Error::Invalid("timeline_missing"))?
                .progress
                .last_complete,
            at: progress.at,
            target: progress.target.unwrap_or(progress.at),
            next_serial: progress.serial - 1,
            page_limit: limit,
            remaining_pages: pages as u32,
            remaining_bytes: pages as u64 * CONTINUATION_PAGE_BYTES,
        };
        capacity::admit_with_reserve(
            &self.io.dir,
            self.io.root.capacity,
            0,
            false,
            c.remaining_bytes,
        )?;
        Ok(Some(c))
    }
}
impl Persistence {
    pub(super) fn next_continuation(
        &self,
        plan: Option<&Continuation>,
        record: &JournalRecord,
    ) -> Result<Option<Continuation>, Error> {
        let Some(plan) = plan else {
            return Ok(self.root.continuation.clone());
        };
        let Ok(receipt) = record.outcome else {
            return Ok(self.root.continuation.clone());
        };
        let progress = receipt
            .timeline
            .ok_or(Error::Invalid("timeline_receipt_missing"))?;
        if progress.at != plan.at || progress.serial != plan.next_serial + 1 {
            return Err(Error::Invalid("timeline_ticket_progress"));
        }
        if progress.phase.is_none() {
            return Ok(None);
        }
        if plan.remaining_pages <= 1 {
            return Err(Error::Invalid("timeline_reservation_exhausted"));
        }
        let mut next = plan.clone();
        next.remaining_pages -= 1;
        next.remaining_bytes = u64::from(next.remaining_pages) * CONTINUATION_PAGE_BYTES;
        next.next_serial = progress.serial;
        Ok(Some(next))
    }
}
