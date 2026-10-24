//! One explicitly registered, trusted producer; durable pending before world acknowledgement.
use super::*;
use mycity_core::{
    inbox::{Command, ExternalFact},
    transaction::Action,
};
pub const OUTBOX_LIMIT: u64 = 64 * 1024;
pub const PENDING_LIMIT: usize = 16;
const NAME: &str = "producer-outbox.json";
#[derive(Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct Queue {
    schema: String,
    scope: mycity_core::arena::Scope,
    next_event: u64,
    pending: Vec<ExternalFact>,
}
#[derive(Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct FileRecord {
    checksum: String,
    payload: Queue,
}

pub(super) const fn capacity_metadata_headers() -> usize {
    std::mem::size_of::<FileRecord>()
}
#[derive(Debug, Serialize)]
pub struct SourceStatus {
    pub registered: bool,
    pub reconciled: bool,
    pub next_event: u64,
    pub pending: Vec<ExternalFact>,
    pub pending_limit: usize,
}
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum SourcePoint {
    Queued,
    WorldCommitted,
    Acknowledged,
}
fn read(dir: &Path) -> Result<Queue, Error> {
    let file: FileRecord = serde_json::from_slice(&read_file(&dir.join(NAME), OUTBOX_LIMIT)?)?;
    if file.checksum != sha256(&json(&file.payload)?)
        || file.payload.schema != "mycity-m0-producer-v1"
        || file.payload.pending.len() > PENDING_LIMIT
    {
        return Err(Error::Invalid("producer_checksum_or_limit"));
    }
    if file.payload.next_event == 0 || file.payload.next_event > u64::from(u32::MAX) + 1 {
        return Err(Error::Invalid("producer_frontier"));
    }
    for (i, f) in file.payload.pending.iter().enumerate() {
        if f.source != 1
            || f.event == 0
            || u64::from(f.event) >= file.payload.next_event
            || i > 0 && file.payload.pending[i - 1].event >= f.event
        {
            return Err(Error::Invalid("producer_order"));
        }
    }
    Ok(file.payload)
}
fn save(dir: &Path, q: Queue, policy: CapacityPolicy) -> Result<(), Error> {
    let bytes = json(&FileRecord {
        checksum: sha256(&json(&q)?),
        payload: q,
    })?;
    if bytes.len() as u64 > OUTBOX_LIMIT {
        return Err(Error::Invalid("producer_file_limit"));
    }
    capacity::admit(dir, policy, bytes.len() as u64 + META_LIMIT, true)?;
    atomic_file(dir, NAME, &bytes, || Ok(()))
}
pub(super) fn exists(dir: &Path) -> bool {
    dir.join(NAME).exists()
}
pub(super) fn verify(dir: &Path, scope: mycity_core::arena::Scope) -> Result<(), Error> {
    if exists(dir) && read(dir)?.scope != scope {
        return Err(Error::Invalid("producer_wrong_scope"));
    }
    Ok(())
}
pub(super) fn validate_attempt(
    dir: &Path,
    registered: bool,
    env: &Envelope,
    current_revision: u64,
) -> Result<(), Error> {
    let facts: Vec<_> = env
        .actions
        .iter()
        .filter_map(|a| {
            if let Action::Inbox {
                command: Command::External { fact },
            } = a
            {
                Some(*fact)
            } else {
                None
            }
        })
        .collect();
    if facts.is_empty() {
        return Ok(());
    }
    if !registered
        || facts.len() != 1
        || env.actions.len() != 1
        || env.operation_id != facts[0].operation_id()
    {
        return Err(Error::FactBlocked(
            "source_requires_registered_canonical_pending",
        ));
    }
    let q = read(dir)?;
    if q.pending.first() != Some(&facts[0]) || env.expected_revision != current_revision {
        return Err(Error::FactBlocked("source_pending_or_attempt_revision"));
    }
    Ok(())
}
impl Store {
    pub fn register_external_source(&mut self) -> Result<(), Error> {
        self.engine()?;
        if !self
            .engine
            .world()
            .view()
            .exchange
            .is_some_and(|e| e.inbox.enabled)
        {
            return Err(Error::Invalid("source_requires_enabled_inbox"));
        }
        if !self.io.root.source_registered {
            capacity::admit(
                &self.io.dir,
                self.io.root.capacity,
                OUTBOX_LIMIT + 2 * META_LIMIT,
                false,
            )?;
        }
        if !exists(&self.io.dir) {
            save(
                &self.io.dir,
                Queue {
                    schema: "mycity-m0-producer-v1".into(),
                    scope: self.engine.world().view().scope,
                    next_event: 1,
                    pending: vec![],
                },
                self.io.root.capacity,
            )?;
        }
        if !self.io.root.source_registered {
            let result = (|| {
                let mut root = self.io.root.clone();
                root.source_registered = true;
                root.generation = root
                    .generation
                    .checked_add(1)
                    .ok_or(Error::Invalid("publication_overflow"))?;
                let slot = (root.generation % 2) as u8;
                self.io.metadata.small.encode(&root, META_LIMIT as usize)?;
                let bytes = self.io.metadata.small.as_slice();
                atomic_file(&self.io.dir, &format!("root-{slot}.json"), bytes, || Ok(()))?;
                let current = Current {
                    schema: "mycity-m0-current-v1".into(),
                    slot,
                    root_sha256: sha256(bytes),
                };
                self.io
                    .metadata
                    .small
                    .encode(&current, META_LIMIT as usize)?;
                atomic_file(
                    &self.io.dir,
                    "CURRENT.json",
                    self.io.metadata.small.as_slice(),
                    || Ok(()),
                )?;
                self.io.root = root;
                Ok::<(), Error>(())
            })();
            if result.is_err() {
                self.io.poisoned = true;
            }
            result?;
        }
        self.io.source_reconciled = false;
        Ok(())
    }
    pub fn source_status(&self) -> Result<SourceStatus, Error> {
        self.engine()?;
        let registered = exists(&self.io.dir);
        let q = if registered {
            read(&self.io.dir)?
        } else {
            Queue {
                schema: String::new(),
                scope: self.engine.world().scope_revision().0,
                next_event: 1,
                pending: vec![],
            }
        };
        Ok(SourceStatus {
            registered,
            reconciled: self.io.source_reconciled,
            next_event: q.next_event,
            pending: q.pending,
            pending_limit: PENDING_LIMIT,
        })
    }
    /// The producer must persist this before declaring its external event committed.
    /// If no durable pending space exists, the producer must pause its own source.
    pub fn queue_external(&mut self, fact: ExternalFact) -> Result<(), Error> {
        self.queue_external_with_observer(fact, |_| Ok(()))
    }
    pub fn queue_external_with_observer(
        &mut self,
        fact: ExternalFact,
        mut observe: impl FnMut(SourcePoint) -> io::Result<()>,
    ) -> Result<(), Error> {
        self.engine()?;
        if !self.io.root.source_registered {
            return Err(Error::Invalid("source_not_registered"));
        }
        let mut q = read(&self.io.dir)?;
        if let Some(old) = q.pending.iter().find(|f| f.event == fact.event) {
            return if *old == fact {
                Ok(())
            } else {
                Err(Error::Invalid("source_event_conflict"))
            };
        }
        let view = self.engine.world().view();
        if fact.source != 1
            || u64::from(fact.event) != q.next_event
            || fact.debtor == fact.creditor
            || fact.amount == mycity_core::units::Money::ZERO
            || !view.accounts.iter().any(|a| a.id == fact.debtor)
            || !view.accounts.iter().any(|a| a.id == fact.creditor)
        {
            return Err(Error::Invalid("source_fact"));
        }
        if q.pending.len() == PENDING_LIMIT {
            return Err(Error::FactBlocked("producer_pending_full"));
        }
        q.pending.push(fact);
        q.next_event += 1;
        self.io.source_reconciled = false;
        let result = save(&self.io.dir, q, self.io.root.capacity)
            .and_then(|()| observe(SourcePoint::Queued).map_err(Error::Io));
        if result
            .as_ref()
            .is_err_and(|e| !matches!(e, Error::Capacity { .. }))
        {
            self.io.poisoned = true;
        }
        result
    }
    pub fn receive_external(&mut self, fact: ExternalFact) -> Result<Outcome, Error> {
        self.receive_external_with_observer(fact, |_| Ok(()))
    }
    pub fn receive_external_with_observer(
        &mut self,
        fact: ExternalFact,
        mut observe: impl FnMut(SourcePoint) -> io::Result<()>,
    ) -> Result<Outcome, Error> {
        self.engine()?;
        let mut q = read(&self.io.dir)?;
        let action = Action::Inbox {
            command: Command::External { fact },
        };
        let prior = self.receipt(fact.operation_id())?;
        let expected_revision = if let Some(r) = prior {
            if r.actor != 0 || r.actions[0] != Some(action) {
                return Err(Error::Invalid("source_event_conflict"));
            }
            r.expected_revision
        } else {
            if q.pending.first() != Some(&fact) {
                return Err(Error::FactBlocked("producer_order_or_missing_pending"));
            }
            self.engine.world().view().revision
        };
        let result = self.execute(
            Caller::System,
            Envelope {
                world: self.engine.world().view().scope.world(),
                branch: self.engine.world().view().scope.branch(),
                operation_id: fact.operation_id(),
                expected_revision,
                actions: vec![action],
            },
        )?;
        if result.model.is_err() || !result.has_persistent_receipt {
            return Err(Error::FactBlocked("world_did_not_accept_fact"));
        }
        if let Err(e) = observe(SourcePoint::WorldCommitted) {
            self.io.poisoned = true;
            return Err(e.into());
        }
        if let Some(i) = q.pending.iter().position(|f| *f == fact) {
            q.pending.remove(i);
            if let Err(e) = save(&self.io.dir, q, self.io.root.capacity) {
                if !matches!(e, Error::Capacity { .. }) {
                    self.io.poisoned = true;
                }
                return Err(e);
            }
        }
        if let Err(e) = observe(SourcePoint::Acknowledged) {
            self.io.poisoned = true;
            return Err(e.into());
        }
        Ok(result)
    }
    /// Upstream's independently known frontier is required on every reopening. A
    /// locally empty queue alone cannot prove no source event was missed during I/O failure.
    pub fn reconcile_external_source(&mut self, upstream_next_event: u64) -> Result<(), Error> {
        self.engine()?;
        let q = read(&self.io.dir)?;
        let applied = self
            .engine
            .world()
            .view()
            .exchange
            .ok_or(Error::Invalid("source_exchange"))?
            .inbox
            .external_next;
        if !q.pending.is_empty() || q.next_event != upstream_next_event || applied != q.next_event {
            return Err(Error::SourceSyncRequired);
        }
        self.io.source_reconciled = true;
        Ok(())
    }
}
