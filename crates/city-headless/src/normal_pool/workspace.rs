//! Named, fixed owner table. Empty entries retain their physical pages at Pool.
//! Only the broker's real Sim Child supervisor can retire a live Sim workspace.
use super::*;

pub(super) const BYTES: usize = 59 * MIB;
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "snake_case")]
pub enum Kind {
    #[default]
    SimCodec,
    HelperCompare,
    Model,
}
impl Kind {
    pub(crate) const fn bytes(self) -> usize {
        match self {
            Self::SimCodec => BYTES,
            Self::HelperCompare => 6 * MIB,
            Self::Model => 20 * MIB,
        }
    }
}
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
enum Phase {
    Prepared,
    Filled,
    Retiring,
    Returned,
}
#[derive(Clone, Copy)]
pub(super) struct Entry {
    generation: u64,
    kind: Kind,
    phase: Phase,
    pid: Option<u32>,
    identity: Option<worker_slots::Identity>,
    released_ns: u64,
    start_page: usize,
    arena: Option<mycity_role_allocator::Snapshot>,
    backing_sha256: Option<Text<64>>,
}
#[derive(Serialize)]
struct Fact {
    schema: &'static str,
    kind: &'static str,
    observed_ns: u64,
    generation: u64,
    owner_kind: Kind,
    phase: Phase,
    pid: Option<u32>,
    identity: Option<worker_slots::Identity>,
    capacity_bytes: usize,
    transition_ns: u64,
    summary: Summary,
    pool_touched_bytes: usize,
    arena: Option<mycity_role_allocator::Snapshot>,
    backing_sha256: Option<Text<64>>,
}
pub(super) struct Filled {
    pub arena: mycity_role_allocator::Snapshot,
    pub digest: Text<64>,
}
impl Pool {
    pub(super) fn workspace_bytes(&self) -> usize {
        self.workspaces
            .iter()
            .flatten()
            .map(|e| e.kind.bytes())
            .sum()
    }
    fn workspace_event(&mut self, kind: &'static str, e: Entry) -> Result<(), String> {
        self.write_event_bounded(
            &Fact {
                schema: "mycity-normal-role-arena-v2",
                kind,
                observed_ns: self.now(),
                generation: e.generation,
                owner_kind: e.kind,
                phase: e.phase,
                pid: e.pid,
                identity: e.identity,
                capacity_bytes: e.kind.bytes(),
                transition_ns: self.now() - e.released_ns,
                summary: self.summary,
                pool_touched_bytes: self.pages.iter().flatten().map(Page::capacity).sum(),
                arena: e.arena,
                backing_sha256: e.backing_sha256,
            },
            2048,
        )
    }
    pub(super) fn prepare_codec(&mut self, generation: u64, kind: Kind) -> Result<(), String> {
        if self
            .workspaces
            .iter()
            .flatten()
            .any(|e| e.generation == generation)
        {
            return Err("normal_workspace_duplicate_prepare".into());
        }
        self.scan()?;
        let slot = self
            .workspaces
            .iter()
            .position(Option::is_none)
            .ok_or("normal_workspace_table_full")?;
        let count = kind.bytes() / MIB;
        let start_page = self.pages[..WORKER_START_PAGE]
            .windows(count)
            .position(|pages| pages.iter().all(Option::is_some))
            .ok_or("normal_workspace_page_overlap")?;
        for page in &mut self.pages[start_page..start_page + count] {
            page.take();
        }
        let e = Entry {
            generation,
            kind,
            phase: Phase::Prepared,
            pid: None,
            identity: None,
            released_ns: self.now(),
            start_page,
            arena: None,
            backing_sha256: None,
        };
        self.workspaces[slot] = Some(e);
        self.summary.workspace_capacity_bytes += kind.bytes();
        self.summary.workspace_grants += 1;
        self.summary.available_touched_bytes -= kind.bytes();
        self.summary.granted_capacity_bytes += kind.bytes();
        self.scan()?;
        self.workspace_event(
            match e.kind {
                Kind::SimCodec => "prepared_before_sim_spawn",
                Kind::HelperCompare | Kind::Model => "prepared_before_helper_spawn",
            },
            e,
        )
    }
    pub(super) fn codec_ready(&self, generation: u64, pid: u32) -> bool {
        self.workspaces.iter().flatten().any(|e| {
            e.generation == generation
                && e.pid == Some(pid)
                && e.phase == Phase::Filled
                && e.identity.is_some()
        })
    }
    pub(super) fn bind_codec(&mut self, generation: u64, pid: u32) -> Result<(), ActionError> {
        let slot = self
            .workspaces
            .iter()
            .position(|e| e.is_some_and(|e| e.generation == generation))
            .ok_or(ActionError::Rejected("normal_workspace_generation"))?;
        let e = self.workspaces[slot].as_mut().expect("found");
        if e.pid.is_some_and(|old| old != pid) {
            return Err(ActionError::Rejected("normal_workspace_pid"));
        }
        e.pid = Some(pid);
        Ok(())
    }
    pub(super) fn fill_codec(
        &mut self,
        generation: u64,
        identity: worker_slots::Identity,
        capacity: usize,
        touched: usize,
        balanced: bool,
        backing: Filled,
    ) -> Result<(), ActionError> {
        let Filled {
            arena,
            digest: backing_sha256,
        } = backing;
        let slot = self
            .workspaces
            .iter()
            .position(|e| e.is_some_and(|e| e.generation == generation))
            .ok_or(ActionError::Rejected("normal_workspace_generation"))?;
        let mut e = self.workspaces[slot].expect("found");
        if e.phase == Phase::Retiring {
            return Err(ActionError::Rejected("normal_workspace_retiring"));
        }
        if e.pid.is_none() {
            return Err(ActionError::Rejected("normal_workspace_sim_unbound"));
        }
        if e.pid != Some(identity.process_pid)
            || identity.process_instance_unix_ns == 0
            || identity.run_epoch != generation
            || identity.job != 1
            || identity.generation != 1
            || identity.attempt != 1
            || capacity != e.kind.bytes()
            || touched != e.kind.bytes()
            || !balanced
        {
            return Err(ActionError::Rejected("normal_workspace_fill_identity"));
        }
        let a = arena
            .activation
            .ok_or(ActionError::Rejected("normal_workspace_arena_absent"))?;
        if a.pid != identity.process_pid
            || a.grant_bytes != e.kind.bytes()
            || a.arena_start == 0
            || a.initialized_bytes != a.arena_bytes
            || a.arena_bytes
                .checked_add(a.bootstrap_charged_bytes)
                .and_then(|n| n.checked_add(a.adapter_header_bytes))
                != Some(e.kind.bytes())
            || arena
                .bootstrap_peak_bytes
                .checked_add(a.adapter_header_bytes)
                .is_none_or(|n| n > MIB)
            || a.adapter_header_bytes != std::mem::size_of::<mycity_role_allocator::Allocator>()
            || a.arena_start.checked_add(a.arena_bytes).is_none()
            || a.bootstrap_charged_bytes > arena.bootstrap_peak_bytes
            || arena.bootstrap_live_bytes > a.bootstrap_charged_bytes
            || a.bootstrap_charged_bytes
                .checked_sub(arena.bootstrap_live_bytes)
                != Some(arena.bootstrap_quarantined_bytes)
            || arena.arena_used_bytes.checked_add(arena.arena_free_bytes)
                != Some(arena.arena_usable_bytes)
            || arena.current_requested_bytes > arena.lifetime_peak_requested_bytes
            || arena.lifetime_peak_requested_bytes > e.kind.bytes()
            || arena.allocation_failures != 0
            || arena.arena_used_bytes > arena.arena_usable_bytes
            || arena.arena_usable_bytes > a.arena_bytes
            || arena.current_requested_bytes > e.kind.bytes()
            || backing_sha256.as_str().len() != 64
            || !backing_sha256
                .as_str()
                .bytes()
                .all(|b| b.is_ascii_hexdigit())
        {
            return Err(ActionError::Rejected("normal_workspace_arena_identity"));
        }
        if let Some(old) = e.identity {
            return if old == identity
                && e.backing_sha256 == Some(backing_sha256)
                && e.arena.and_then(|a| a.activation) == arena.activation
            {
                Ok(())
            } else {
                Err(ActionError::Rejected("normal_workspace_changed_identity"))
            };
        }
        e.identity = Some(identity);
        e.arena = Some(arena);
        e.backing_sha256 = Some(backing_sha256);
        e.phase = Phase::Filled;
        self.workspaces[slot] = Some(e);
        self.summary.workspace_fills += 1;
        self.summary.maximum_prepare_to_fill_or_reap_ns = self
            .summary
            .maximum_prepare_to_fill_or_reap_ns
            .max(self.now() - e.released_ns);
        self.workspace_event(
            match e.kind {
                Kind::SimCodec => "sim_actual_codec_slots_filled",
                Kind::HelperCompare | Kind::Model => "helper_actual_compare_slots_filled",
            },
            e,
        )?;
        Ok(())
    }
    /// Caller is exclusively broker Cancel-before-spawn / Revoke-after-Child-wait.
    pub(super) fn retire_codec(&mut self, generation: u64) -> Result<(), String> {
        let slot = self
            .workspaces
            .iter()
            .position(|e| e.is_some_and(|e| e.generation == generation))
            .ok_or("normal_workspace_retire_generation")?;
        let mut e = self.workspaces[slot].expect("found");
        e.phase = Phase::Retiring;
        self.workspaces[slot] = Some(e);
        self.workspace_event(
            match e.kind {
                Kind::SimCodec => "retiring_after_sim_wait_or_cancel",
                Kind::HelperCompare | Kind::Model => "retiring_after_helper_wait_or_cancel",
            },
            e,
        )?;
        let start = e.start_page;
        let end = start + e.kind.bytes() / MIB;
        // Do not publish credit until every replacement page has been touched.
        for i in start..end {
            if self.pages[i].is_some() {
                return Err("normal_workspace_duplicate_return".into());
            }
            if i == start + 1 && self.fault == Some(Fault::WorkspaceReturnSecondPage) {
                self.fault = None;
                self.workspace_event("injected_partial_workspace_return_failure", e)?;
                return Err("normal_workspace_injected_return_failure".into());
            }
            self.pages[i] = Some(self.make_page()?);
        }
        self.workspaces[slot] = None;
        self.summary.workspace_capacity_bytes -= e.kind.bytes();
        self.summary.workspace_returns += 1;
        self.summary.available_touched_bytes += e.kind.bytes();
        self.summary.granted_capacity_bytes -= e.kind.bytes();
        if e.identity.is_none() {
            self.summary.maximum_prepare_to_fill_or_reap_ns = self
                .summary
                .maximum_prepare_to_fill_or_reap_ns
                .max(self.now() - e.released_ns);
        }
        self.scan()?;
        e.phase = Phase::Returned;
        self.workspace_event(
            match e.kind {
                Kind::SimCodec => "returned_after_sim_wait_or_cancel",
                Kind::HelperCompare | Kind::Model => "returned_after_helper_wait_or_cancel",
            },
            e,
        )
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    fn pool() -> (PathBuf, Pool) {
        let root = std::env::temp_dir().join(format!(
            "normal-workspaces-{}-{}",
            std::process::id(),
            secret().unwrap().as_str()
        ));
        fs::create_dir(&root).unwrap();
        let pool = Pool::new(&root).unwrap();
        (root, pool)
    }
    fn arena_fixture() -> mycity_role_allocator::Snapshot {
        let a = mycity_role_allocator::Allocator::new();
        a.activate(BYTES).unwrap();
        a.snapshot()
    }
    #[test]
    fn sim_and_worker_use_distinct_real_pages_and_do_not_return_each_others_capacity() {
        let (root, mut p) = pool();
        p.prepare_codec(1, Kind::SimCodec).unwrap();
        assert_eq!(
            p.summary.available_touched_bytes,
            super::super::BYTES - BYTES
        );
        let codec = worker_slots::Owner::new(1, 1, 1, 1).unwrap();
        let arena = arena_fixture();
        let digest = Text::new(&"a".repeat(64)).unwrap();
        let r = codec.report().unwrap();
        assert!(p
            .fill_codec(
                1,
                codec.identity(),
                r.owner_capacity_bytes,
                r.available_touched_bytes,
                true,
                Filled { arena, digest }
            )
            .is_err());
        assert!(!p.codec_ready(1, std::process::id()));
        p.bind_codec(1, std::process::id()).unwrap();
        assert!(!p.codec_ready(1, std::process::id()));
        let mut overflow = arena;
        overflow.bootstrap_peak_bytes = usize::MAX;
        assert!(p
            .fill_codec(
                1,
                codec.identity(),
                BYTES,
                BYTES,
                true,
                Filled {
                    arena: overflow,
                    digest
                }
            )
            .is_err());
        let mut wrong = arena;
        wrong.activation.as_mut().unwrap().arena_start = usize::MAX;
        assert!(p
            .fill_codec(
                1,
                codec.identity(),
                BYTES,
                BYTES,
                true,
                Filled {
                    arena: wrong,
                    digest
                }
            )
            .is_err());
        p.fill_codec(
            1,
            codec.identity(),
            BYTES,
            BYTES,
            true,
            Filled { arena, digest },
        )
        .unwrap();
        p.fill_codec(
            1,
            codec.identity(),
            BYTES,
            BYTES,
            true,
            Filled { arena, digest },
        )
        .unwrap();
        assert_eq!(p.summary.workspace_fills, 1);
        assert!(p
            .fill_codec(
                1,
                codec.identity(),
                BYTES,
                BYTES,
                true,
                Filled {
                    arena,
                    digest: Text::new(&"b".repeat(64)).unwrap()
                }
            )
            .is_err());
        assert!(p.codec_ready(1, std::process::id()));
        let key = secret().unwrap();
        let (id, _) = p
            .legacy_action(Op::Prepare {
                parent: std::process::id(),
                parent_instance: heap_meter::process_instance(),
                key,
            })
            .unwrap();
        let active = p.active.unwrap();
        p.legacy_action(Op::Attach {
            id: id.unwrap(),
            key,
            pid: std::process::id(),
        })
        .unwrap();
        let worker = worker_slots::Owner::new(2, 1, 1, 1).unwrap();
        let r = worker.report().unwrap();
        let air = mycity_core::air::AirWorkspace::new().unwrap();
        let typed = air.report().unwrap();
        p.legacy_action(Op::Filled {
            id: id.unwrap(),
            key: active.worker_key,
            identity: worker.identity(),
            capacity_bytes: r.owner_capacity_bytes,
            available_touched_bytes: r.available_touched_bytes,
            full_balance_scan_passed: true,
            typed_instance: typed.workspace_instance,
            typed_capacity: typed.payload_capacity_bytes,
            typed_scan_passed: typed.full_fill_scan_passed,
        })
        .unwrap();
        assert_eq!(p.summary.granted_capacity_bytes, BYTES + GRANT_BYTES);
        assert_eq!(
            p.summary.available_touched_bytes
                + BYTES
                + worker.report().unwrap().owner_capacity_bytes
                + air.report().unwrap().payload_capacity_bytes,
            super::super::BYTES
        );
        drop(air);
        drop(worker); // local allocation test; real Child waits are covered by broker integration.
        p.give_back(p.active.unwrap()).unwrap();
        assert_eq!(p.summary.workspace_capacity_bytes, BYTES);
        assert_eq!(p.summary.granted_capacity_bytes, BYTES);
        drop(codec);
        p.retire_codec(1).unwrap();
        assert_eq!(p.summary.available_touched_bytes, super::super::BYTES);
        assert_eq!(p.summary.workspace_grants, p.summary.workspace_returns);
        p.scan().unwrap();
        drop(p);
        fs::remove_dir_all(root).unwrap();
    }
    #[test]
    fn bounded_table_overflow_duplicate_generation_and_partial_refill_fail_closed() {
        let (root, mut p) = pool();
        for generation in 1..=4 {
            p.prepare_codec(generation, Kind::HelperCompare).unwrap();
        }
        assert!(p.prepare_codec(1, Kind::SimCodec).is_err());
        assert!(p.prepare_codec(5, Kind::HelperCompare).is_err());
        assert!(p.retire_codec(5).is_err());
        for generation in 1..=4 {
            p.retire_codec(generation).unwrap();
        }
        assert_eq!(p.summary.available_touched_bytes, super::super::BYTES);
        p.prepare_codec(5, Kind::HelperCompare).unwrap();
        p.fault = Some(Fault::WorkspaceReturnSecondPage);
        assert!(p.retire_codec(5).is_err());
        assert_eq!(p.summary.workspace_returns, 4);
        assert_eq!(
            p.summary.granted_capacity_bytes,
            Kind::HelperCompare.bytes()
        );
        assert_eq!(p.workspaces[0].unwrap().phase, Phase::Retiring);
        assert!(p.scan().is_err()); // partial pages cannot be presented as full balance.
        drop(p);
        fs::remove_dir_all(root).unwrap();
    }
}
