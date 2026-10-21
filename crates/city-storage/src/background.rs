//! Sim-owned immutable air assets and a finite normal-funded background obligation.
use super::*;
use mycity_core::{
    air::{BackgroundStateSlot, State},
    background::{Command as B, ASSET_LIMIT},
    transaction::Action,
};
pub const BACKGROUND_SLOTS: u8 = 16;
pub const WORKER_CACHE_BYTES: u64 = 6 * 1024 * 1024;
pub const BACKGROUND_ASSET_RESERVE: u64 = 10 * 1024 * 1024;
pub const BACKGROUND_LIVE_ASSETS: usize = 32;
const PREFIX: &str = "air-result-";
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct BackgroundReserve {
    pub start: u64,
    pub at: u64,
    pub slots: u8,
    pub staged_sha256: Option<[u8; 32]>,
    pub staged_bytes: u64,
    #[serde(default)]
    pub worker_cache_charge: u64,
}
impl BackgroundReserve {
    pub fn remaining_bytes(&self) -> u64 {
        BACKGROUND_ASSET_RESERVE - self.staged_bytes - self.worker_cache_charge
            + u64::from(self.slots) * CONTINUATION_PAGE_BYTES
    }
    pub(super) fn valid(&self) -> bool {
        matches!(self.worker_cache_charge, 0 | WORKER_CACHE_BYTES)
            && self.start < self.at
            && self.at - self.start <= 3600
            && self.slots > 0
            && self.slots <= BACKGROUND_SLOTS
            && self.staged_bytes <= ASSET_LIMIT as u64
            && self.staged_sha256.is_some() == (self.staged_bytes > 0)
    }
}
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum BackgroundPoint {
    Validated,
    AssetHalf,
    AssetSynced,
    AssetPublished,
    StagedRootPublished,
    StagedCurrentRenamed,
    StagedCurrentSynced,
}
pub(super) struct Plan {
    pub initial: Option<BackgroundReserve>,
    pub funded: bool,
}
fn asset_name(hash: &[u8; 32]) -> String {
    format!("{PREFIX}{}.air", hex(hash))
}
pub(super) fn successful_hash(envelope: &Envelope, success: bool) -> Option<[u8; 32]> {
    if !success {
        return None;
    }
    match envelope.actions.as_slice() {
        [Action::Background {
            command: B::Accept {
                candidate_sha256, ..
            },
        }] => Some(*candidate_sha256),
        _ => None,
    }
}
pub(super) fn load_asset_into(
    dir: &Path,
    hash: &[u8; 32],
    slot: &mut BackgroundStateSlot,
) -> Result<(), Error> {
    slot.reset_preserving_capacity();
    let result = (|| {
        let path = dir.join(asset_name(hash));
        let expected = stream::Identity {
            bytes: fs::symlink_metadata(&path)?.len(),
            sha256: hex(hash),
        };
        stream::verified(&path, &expected, ASSET_LIMIT as u64, |reader| {
            slot.restore_reader_into(reader).map_err(Error::Model)
        })?;
        if !slot.state().map_err(Error::Model)?.complete() {
            return Err(Error::Invalid("background_asset_partial"));
        }
        Ok(())
    })();
    if result.is_err() {
        slot.reset_preserving_capacity();
    }
    result
}
pub(super) fn remember(live: &mut Vec<[u8; 32]>, hash: [u8; 32]) -> Result<(), Error> {
    if !live.contains(&hash) {
        if live.len() == BACKGROUND_LIVE_ASSETS {
            return Err(Error::Invalid("background_live_asset_limit"));
        }
        live.push(hash);
    }
    Ok(())
}
/// Call only after validating CURRENT, checkpoints and every committed replay dependency.
pub(super) fn gc(dir: &Path, root: &Root, live: &[[u8; 32]]) -> Result<(), Error> {
    let staged = root
        .background_reserve
        .as_ref()
        .and_then(|r| r.staged_sha256);
    let mut cleaned = false;
    for entry in fs::read_dir(dir)? {
        let entry = entry?;
        let name = entry.file_name();
        let name = name.to_string_lossy();
        let owned_asset = name
            .strip_prefix(PREFIX)
            .and_then(|s| s.strip_suffix(".air"));
        let owned_temp = name.starts_with(".air-import-") && name.ends_with(".tmp");
        if let Some(hash) = owned_asset {
            if hash.len() != 64
                || !hash
                    .bytes()
                    .all(|b| b.is_ascii_digit() || (b'a'..=b'f').contains(&b))
            {
                continue;
            }
            let retained =
                staged.is_some_and(|s| hex(&s) == hash) || live.iter().any(|s| hex(s) == hash);
            if retained {
                continue;
            }
        } else if !owned_temp {
            continue;
        }
        if !entry.file_type()?.is_file() {
            return Err(Error::Invalid("background_gc_nonregular"));
        }
        fs::remove_file(entry.path())?;
        cleaned = true;
    }
    if cleaned {
        sync_directory(dir)?;
    }
    Ok(())
}
impl Store {
    pub fn background_reserve(&self) -> Result<Option<BackgroundReserve>, Error> {
        self.engine()?;
        Ok(self.io.root.background_reserve.clone())
    }
    pub(super) fn validate_background(
        engine: &Engine,
        dir: &Path,
        root: &Root,
        candidate: &mut BackgroundStateSlot,
    ) -> Result<(), Error> {
        let required = engine.background().is_some()
            && engine.timeline().is_some_and(|t| {
                t.progress.phase.is_some()
                    && t.progress.at > t.progress.last_complete
                    && engine.background().is_some_and(|b| {
                        b.pending().is_some() || u64::from(b.time()) < t.progress.at
                    })
            });
        match (&root.background_reserve, required) {
            (None, false) => Ok(()),
            (Some(r), true) if r.valid() => {
                let p = engine.timeline().expect("required timeline").progress;
                if r.start != p.last_complete || r.at != p.at {
                    return Err(Error::Invalid("background_reserve_window"));
                }
                if let Some(hash) = r.staged_sha256 {
                    load_asset_into(dir, &hash, candidate)?;
                    let state = candidate.state().map_err(Error::Model)?;
                    if state.checkpoint_digest().map_err(Error::Model)?.0 as u64 != r.staged_bytes {
                        return Err(Error::Invalid("background_staged_length"));
                    }
                    if !engine
                        .background_candidate_matches(state)
                        .map_err(Error::Model)?
                    {
                        return Err(Error::Invalid("background_staged_input"));
                    }
                }
                Ok(())
            }
            _ => Err(Error::Invalid("background_reserve_state")),
        }
    }
    pub(super) fn background_attachment(
        &mut self,
        caller: Caller,
        envelope: &Envelope,
        trace: &mut mycity_core::diagnostics::Trace<'_>,
    ) -> Result<bool, Error> {
        match (
            envelope.actions.as_slice(),
            self.io.root.background_reserve.as_ref(),
        ) {
            (
                [Action::Background {
                    command:
                        B::Accept {
                            candidate_sha256, ..
                        },
                }],
                Some(r),
            ) if r.staged_sha256 == Some(*candidate_sha256) => {
                load_asset_into(&self.io.dir, candidate_sha256, &mut self.air_candidate)?;
                let state = self.air_candidate.state().map_err(Error::Model)?;
                // Failure receipts are evaluated without attachments, hence their replay
                // never depends on a payload that was not admitted into the model.
                if self.engine.background_control_quote_diagnosed(
                    caller,
                    envelope,
                    Some(state),
                    trace,
                ) {
                    Ok(true)
                } else {
                    Ok(false)
                }
            }
            _ => Ok(false),
        }
    }
    pub(super) fn background_funded_quote(
        &self,
        caller: Caller,
        env: &Envelope,
        candidate: Option<&State>,
        trace: &mut mycity_core::diagnostics::Trace<'_>,
    ) -> bool {
        let restart_needed = match env.actions.as_slice() {
            [Action::Background {
                command: B::Restart,
            }] => self
                .engine
                .background()
                .is_some_and(|b| b.incarnation() == 0 || b.pending().is_some_and(|j| j.running)),
            _ => true,
        };
        self.io.root.background_reserve.is_some()
            && restart_needed
            && self
                .engine
                .background_control_quote_diagnosed(caller, env, candidate, trace)
    }
    pub(super) fn prepare_background(
        &mut self,
        caller: Caller,
        env: &Envelope,
        continuation: Option<&Continuation>,
        funded: bool,
        trace: &mut mycity_core::diagnostics::Trace<'_>,
    ) -> Result<Plan, Error> {
        let initial = if self.engine.background().is_some()
            && self.io.root.background_reserve.is_none()
            && self
                .engine
                .timeline()
                .is_some_and(|t| t.progress.phase.is_none())
        {
            if let Some(c) = continuation.filter(|c| c.at > c.start) {
                if self.io.background_live.len() >= BACKGROUND_LIVE_ASSETS {
                    self.checkpoint()?;
                }
                Some(BackgroundReserve {
                    start: c.start,
                    at: c.at,
                    slots: BACKGROUND_SLOTS,
                    staged_sha256: None,
                    staged_bytes: 0,
                    worker_cache_charge: self.io.root.worker_cache_bytes,
                })
            } else {
                None
            }
        } else {
            None
        };
        let held = initial
            .clone()
            .or_else(|| self.io.root.background_reserve.clone());
        if let Some(r) = held {
            if funded
                && r.slots <= 1
                && !matches!(
                    env.actions.as_slice(),
                    [Action::Background {
                        command: B::Accept { .. }
                    }]
                )
            {
                return Err(Error::Invalid("background_control_reserve_exhausted"));
            }
            let clock = continuation.is_some();
            let existing = self
                .engine
                .can_advance_existing_responsibility_diagnosed(caller, env, trace);
            let floor = usize::from(r.slots) - usize::from(funded)
                + continuation
                    .or(self.io.root.continuation.as_ref())
                    .map_or(0, |c| c.remaining_pages as usize)
                + if clock || existing || funded { 0 } else { 64 };
            if self.engine.journal().len() + 1 + floor > MAX_RECEIPTS {
                if initial.is_some() {
                    self.checkpoint()?;
                } else {
                    return Err(Error::Invalid("background_receipt_reserve"));
                }
            }
            if initial.is_some() {
                let timeline_bytes = continuation
                    .or(self.io.root.continuation.as_ref())
                    .map_or(0, |c| c.remaining_bytes);
                capacity::admit_with_reserve(
                    &self.io.dir,
                    self.io.root.capacity,
                    CONTINUATION_PAGE_BYTES,
                    false,
                    timeline_bytes + r.remaining_bytes(),
                )?;
            }
        }
        Ok(Plan { initial, funded })
    }
    pub fn import_background_candidate(
        &mut self,
        job: u64,
        generation: u32,
        bytes: &[u8],
    ) -> Result<[u8; 32], Error> {
        self.import_background_candidate_with_observer(job, generation, bytes, |_| Ok(()))
    }
    /// Trusted Sim boundary. It copies worker bytes and never retains a worker path.
    pub fn import_background_candidate_with_observer(
        &mut self,
        job: u64,
        generation: u32,
        bytes: &[u8],
        mut observer: impl FnMut(BackgroundPoint) -> io::Result<()>,
    ) -> Result<[u8; 32], Error> {
        self.import_candidate_diagnosed(
            job,
            generation,
            bytes,
            &mut observer,
            &mut mycity_core::diagnostics::Trace::disabled(),
        )
    }
    pub fn import_candidate_diagnosed(
        &mut self,
        job: u64,
        generation: u32,
        bytes: &[u8],
        mut observer: impl FnMut(BackgroundPoint) -> io::Result<()>,
        trace: &mut mycity_core::diagnostics::Trace<'_>,
    ) -> Result<[u8; 32], Error> {
        self.engine()?;
        if bytes.len() > ASSET_LIMIT {
            return Err(Error::Invalid("background_asset_bytes"));
        }
        let hash = digest(bytes);
        self.air_candidate
            .restore_into(bytes)
            .map_err(Error::Model)?;
        let state = self.air_candidate.state().map_err(Error::Model)?;
        let view = self.engine.world().view();
        let envelope = Envelope {
            world: view.scope.world(),
            branch: view.scope.branch(),
            operation_id: mycity_core::background::accept_operation_id(job)
                .map_err(Error::Model)?,
            expected_revision: view.revision,
            actions: vec![Action::Background {
                command: B::Accept {
                    job,
                    generation,
                    candidate_sha256: hash,
                },
            }],
        };
        // Import quotes use the same canonical admission identity as the final transaction.
        if !self.engine.background_control_quote_diagnosed(
            Caller::System,
            &envelope,
            Some(state),
            trace,
        ) {
            return Err(Error::Invalid("background_candidate_rejected"));
        }
        let current = self
            .io
            .root
            .background_reserve
            .as_ref()
            .ok_or(Error::Invalid("background_reserve_missing"))?;
        if let Some(existing) = current.staged_sha256 {
            if existing != hash {
                return Err(Error::Invalid("background_candidate_already_staged"));
            }
            let result = (|| {
                load_asset_into(&self.io.dir, &hash, &mut self.air_candidate)?;
                File::open(self.io.dir.join(asset_name(&hash)))?.sync_all()?;
                sync_directory(&self.io.dir)?;
                Ok(hash)
            })();
            if result.is_err() {
                self.io.poisoned = true;
            }
            return result;
        }
        if current.slots < 2 {
            return Err(Error::Invalid("background_control_reserve_exhausted"));
        }
        let floor = usize::from(current.slots)
            + self
                .io
                .root
                .continuation
                .as_ref()
                .map_or(0, |c| c.remaining_pages as usize);
        if self.engine.journal().len() + floor > MAX_RECEIPTS {
            return Err(Error::Invalid("background_receipt_reserve"));
        }
        if self.io.background_live.len() >= BACKGROUND_LIVE_ASSETS
            && !self.io.background_live.contains(&hash)
        {
            return Err(Error::Invalid("background_live_asset_limit"));
        }
        observer(BackgroundPoint::Validated)?;
        let mut root = self.io.root.clone();
        let staged = root
            .background_reserve
            .as_mut()
            .expect("checked reservation");
        staged.slots -= 1;
        staged.staged_sha256 = Some(hash);
        staged.staged_bytes = bytes.len() as u64;
        let transient_held = self
            .io
            .root
            .continuation
            .as_ref()
            .map_or(0, |c| c.remaining_bytes)
            + current.remaining_bytes()
            - 2 * bytes.len() as u64
            - CONTINUATION_PAGE_BYTES;
        capacity::admit_with_reserve(
            &self.io.dir,
            self.io.root.capacity,
            2 * bytes.len() as u64 + 2 * META_LIMIT,
            true,
            transient_held,
        )?;
        let result = (|| {
            let id = TEMP_ID.fetch_add(1, Ordering::Relaxed);
            let temp = format!(".air-import-{}-{id}.tmp", std::process::id());
            let mut file = OpenOptions::new()
                .create_new(true)
                .write(true)
                .open(self.io.dir.join(&temp))?;
            let half = bytes.len() / 2;
            file.write_all(&bytes[..half])?;
            observer(BackgroundPoint::AssetHalf)?;
            file.write_all(&bytes[half..])?;
            file.sync_all()?;
            observer(BackgroundPoint::AssetSynced)?;
            let target = self.io.dir.join(asset_name(&hash));
            match fs::hard_link(self.io.dir.join(&temp), &target) {
                Ok(()) => (),
                Err(e) if e.kind() == io::ErrorKind::AlreadyExists => {
                    load_asset_into(&self.io.dir, &hash, &mut self.air_candidate)?;
                    File::open(&target)?.sync_all()?;
                }
                Err(e) => return Err(e.into()),
            }
            fs::remove_file(self.io.dir.join(&temp))?;
            sync_directory(&self.io.dir)?;
            observer(BackgroundPoint::AssetPublished)?;
            root.generation = root
                .generation
                .checked_add(1)
                .ok_or(Error::Invalid("publication_overflow"))?;
            let encoded = json(&root)?;
            let slot = (root.generation % 2) as u8;
            atomic_file(&self.io.dir, &format!("root-{slot}.json"), &encoded, || {
                Ok(())
            })?;
            observer(BackgroundPoint::StagedRootPublished)?;
            atomic_file(
                &self.io.dir,
                "CURRENT.json",
                &json(&Current {
                    schema: "mycity-m0-current-v1".into(),
                    slot,
                    root_sha256: sha256(&encoded),
                })?,
                || observer(BackgroundPoint::StagedCurrentRenamed),
            )?;
            observer(BackgroundPoint::StagedCurrentSynced)?;
            self.io.root = root;
            Ok(hash)
        })();
        if result.is_err() {
            self.io.poisoned = true;
        }
        result
    }
}
impl Persistence {
    pub(super) fn next_background(
        &self,
        plan: &Plan,
        env: &Envelope,
        record: &JournalRecord,
    ) -> Result<Option<BackgroundReserve>, Error> {
        if record.outcome.is_err() {
            return Ok(self.root.background_reserve.clone());
        }
        if let Some(initial) = &plan.initial {
            return Ok(Some(initial.clone()));
        }
        if !plan.funded {
            return Ok(self.root.background_reserve.clone());
        }
        if successful_hash(env, true).is_some() {
            return Ok(None);
        }
        let mut next = self
            .root
            .background_reserve
            .clone()
            .ok_or(Error::Invalid("background_reserve_missing"))?;
        next.slots = next
            .slots
            .checked_sub(1)
            .filter(|n| *n > 0)
            .ok_or(Error::Invalid("background_control_reserve_exhausted"))?;
        Ok(Some(next))
    }
}

/// A cache allocation is durable quota, independent of disposable cache contents.
pub(super) fn validate_cache_charge(root: &Root) -> Result<(), Error> {
    if !matches!(root.worker_cache_bytes, 0 | WORKER_CACHE_BYTES)
        || root
            .background_reserve
            .as_ref()
            .is_some_and(|r| !r.valid() || r.worker_cache_charge != root.worker_cache_bytes)
    {
        return Err(Error::Invalid("worker_cache_charge"));
    }
    Ok(())
}
/// Bounded diagnostics only: no cache asset is needed to restore authoritative state.
pub(super) fn cache_actual_bytes(dir: &Path) -> Result<u64, Error> {
    let cache = dir.join("air-worker");
    match fs::symlink_metadata(&cache) {
        Err(e) if e.kind() == io::ErrorKind::NotFound => return Ok(0),
        Err(e) => return Err(e.into()),
        Ok(m) if !m.is_dir() => return Err(Error::Invalid("worker_cache_directory")),
        Ok(_) => (),
    }
    let mut total = 0u64;
    for (n, child) in fs::read_dir(&cache)?.enumerate() {
        if n >= 2 {
            return Err(Error::Invalid("worker_cache_directories"));
        }
        let child = child?;
        if !matches!(child.file_name().to_str(), Some("inputs" | "candidates"))
            || !child.file_type()?.is_dir()
        {
            return Err(Error::Invalid("worker_cache_directory"));
        }
        for (n, asset) in fs::read_dir(child.path())?.enumerate() {
            if n >= 8 {
                return Err(Error::Invalid("worker_cache_files"));
            }
            let asset = asset?;
            if !asset.file_type()?.is_file() {
                return Err(Error::Invalid("worker_cache_asset"));
            }
            total = total
                .checked_add(asset.metadata()?.len())
                .ok_or(Error::Invalid("worker_cache_overflow"))?;
            if total > WORKER_CACHE_BYTES {
                return Err(Error::Invalid("worker_cache_bytes"));
            }
        }
    }
    Ok(total)
}
impl Store {
    /// Transfer the pre-funded IPC allocation into a persistent cache quota.
    /// Call before creating cache files; this neither leases nor dispatches a task.
    pub fn prepare_worker_cache(&mut self) -> Result<PathBuf, Error> {
        self.engine()?;
        if self.io.root.worker_cache_bytes == WORKER_CACHE_BYTES {
            return Ok(self.io.dir.join("air-worker"));
        }
        if self.engine.background().and_then(|b| b.pending()).is_none() {
            return Err(Error::Invalid("worker_cache_no_job"));
        }
        if cache_actual_bytes(&self.io.dir)? != 0 {
            return Err(Error::Invalid("worker_cache_unfunded_files"));
        }
        let mut root = self.io.root.clone();
        let r = root
            .background_reserve
            .as_mut()
            .ok_or(Error::Invalid("background_reserve_missing"))?;
        if r.slots < 2 {
            return Err(Error::Invalid("background_control_reserve_exhausted"));
        }
        r.slots -= 1;
        r.worker_cache_charge = WORKER_CACHE_BYTES;
        root.worker_cache_bytes = WORKER_CACHE_BYTES;
        validate_cache_charge(&root)?;
        root.generation = root
            .generation
            .checked_add(1)
            .ok_or(Error::Invalid("publication_overflow"))?;
        let held = root.continuation.as_ref().map_or(0, |c| c.remaining_bytes)
            + root
                .background_reserve
                .as_ref()
                .expect("checked reserve")
                .remaining_bytes();
        // Existing CURRENT still charges zero; include the incoming charge explicitly.
        capacity::admit_with_reserve(
            &self.io.dir,
            root.capacity,
            WORKER_CACHE_BYTES + 2 * META_LIMIT,
            true,
            held,
        )?;
        let result = (|| {
            let encoded = json(&root)?;
            let slot = (root.generation % 2) as u8;
            atomic_file(&self.io.dir, &format!("root-{slot}.json"), &encoded, || {
                Ok(())
            })?;
            atomic_file(
                &self.io.dir,
                "CURRENT.json",
                &json(&Current {
                    schema: "mycity-m0-current-v1".into(),
                    slot,
                    root_sha256: sha256(&encoded),
                })?,
                || Ok(()),
            )?;
            self.io.root = root;
            Ok(self.io.dir.join("air-worker"))
        })();
        if result.is_err() {
            self.io.poisoned = true;
        }
        result
    }
}

#[cfg(test)]
mod slot_load_tests {
    use super::*;
    #[test]
    fn outer_hash_failure_invalidates_otherwise_valid_decoded_candidate() {
        let dir = std::env::temp_dir().join(format!(
            "mycity-slot-asset-{}-{}",
            std::process::id(),
            TEMP_ID.fetch_add(1, Ordering::Relaxed)
        ));
        fs::create_dir(&dir).unwrap();
        struct Cleanup(PathBuf);
        impl Drop for Cleanup {
            fn drop(&mut self) {
                let _ = fs::remove_dir_all(&self.0);
            }
        }
        let _cleanup = Cleanup(dir.clone());
        let input=mycity_core::air::Input::parse(br#"{"schema":"run36-air-f32-v1","side":128,"target":1,"initial_micro":2000000,"boundary_micro":[2000000,2000000,2000000,2000000],"boundary_until":1,"sources":[]}"#).unwrap();
        let mut state = State::new(input).unwrap();
        while !state.complete() {
            state.page(8192, 1024).unwrap();
        }
        let mut bytes = state.checkpoint().unwrap();
        let hash = digest(&bytes);
        let file = dir.join(asset_name(&hash));
        fs::write(&file, &bytes).unwrap();
        let mut slot = BackgroundStateSlot::new();
        let id = slot.identity();
        load_asset_into(&dir, &hash, &mut slot).unwrap();
        assert!(slot.state().unwrap().complete());
        bytes.push(b' '); // valid inner payload and EOF, but wrong content address.
        assert!(State::restore(&bytes).is_ok());
        fs::write(&file, &bytes).unwrap();
        assert!(load_asset_into(&dir, &hash, &mut slot).is_err());
        assert!(slot.state().is_err());
        assert_eq!(slot.identity(), id);
        bytes.pop();
        fs::write(&file, &bytes).unwrap();
        load_asset_into(&dir, &hash, &mut slot).unwrap();
        assert_eq!(slot.identity(), id);
    }
}
