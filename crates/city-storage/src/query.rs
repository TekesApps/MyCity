//! Snapshot-bound pagination. Querying does not grant mutation or advance time.
use super::*;
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Query {
    Timeline,
    Contracts,
    Fulfilment,
    Arrears,
    Capacity,
    Blocked,
    Recovery,
    Receipts,
}
#[derive(Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct Cursor {
    sequence: u64,
    generation: u64,
    state: String,
    query: Query,
    offset: usize,
}
#[derive(Debug, Serialize)]
pub struct Page {
    pub schema: &'static str,
    pub sequence: u64,
    pub timeline: Option<mycity_core::timeline::Progress>,
    pub complete_boundary: bool,
    pub state_sha256: String,
    pub snapshot_sha256: String,
    pub rows: Vec<serde_json::Value>,
    pub next_cursor: Option<String>,
}
/// Bounds the number of JSON projections, not typed scanning or encoded row bytes.
struct PageRows {
    offset: usize,
    limit: usize,
    total: usize,
    overflow: bool,
    rows: Vec<serde_json::Value>,
}
impl PageRows {
    fn new(offset: usize, limit: usize) -> Self {
        Self {
            offset,
            limit,
            total: 0,
            overflow: false,
            rows: Vec::with_capacity(limit),
        }
    }
    fn emit(&mut self, project: impl FnOnce() -> serde_json::Value) {
        if self.total >= self.offset && self.rows.len() < self.limit {
            self.rows.push(project());
        }
        match self.total.checked_add(1) {
            Some(total) => self.total = total,
            None => self.overflow = true,
        }
    }
    fn finish(self) -> Result<(Vec<serde_json::Value>, usize), Error> {
        if self.overflow {
            return Err(Error::Invalid("query_offset_overflow"));
        }
        if self.offset > self.total {
            return Err(Error::Invalid("query_offset"));
        }
        Ok((self.rows, self.total))
    }
}
impl Store {
    pub fn query(
        &mut self,
        query: Query,
        cursor: Option<&str>,
        limit: usize,
    ) -> Result<Page, Error> {
        self.engine()?;
        if limit == 0 || limit > 64 {
            return Err(Error::Invalid("query_page_limit"));
        }
        let state_hash = self.state_hash()?;
        let hash = sha256(&json(
            &serde_json::json!({"state":state_hash,"source":self.source_status()?,"capacity":self.capacity_status()?}),
        )?);
        let mut offset = 0;
        if let Some(c) = cursor {
            if c.len() > 1024 {
                return Err(Error::Invalid("query_cursor_limit"));
            }
            let c: Cursor = serde_json::from_str(c)?;
            if c.sequence != self.io.root.sequence
                || c.generation != self.io.root.generation
                || c.state != hash
                || c.query != query
            {
                return Err(Error::Invalid("query_snapshot_changed"));
            }
            offset = c.offset;
        }
        let (rows, total) = if query == Query::Receipts {
            self.receipt_rows(offset, limit)?
        } else if query == Query::Timeline {
            let mut rows = Vec::with_capacity(limit);
            let total;
            if let Some(t) = self.engine.timeline() {
                total = 1 + t.claims.len() + t.payroll.len();
                if offset > total {
                    return Err(Error::Invalid("query_offset"));
                }
                for i in offset..offset.saturating_add(limit).min(total) {
                    rows.push(if i==0{serde_json::json!({"type":"timeline","progress":t.progress,"complete":t.progress.target.is_none(),"boundary_complete":t.progress.phase.is_none(),"claim_count":t.claims.len(),"payroll_count":t.payroll.len()})}else if i<=t.claims.len(){serde_json::json!({"type":"claim","value":t.claims[i-1]})}else{serde_json::json!({"type":"payroll","value":t.payroll[i-1-t.claims.len()]})});
                }
            } else {
                total = 0;
            }

            (rows, total)
        } else {
            // Hold immutable typed references only for this synchronous call. Never
            // clone WorldView or project skipped rows into temporary JSON values.
            let world = self.engine.world();
            let region = world.query_region();
            let exchange = world.query_exchange();
            let mut page = PageRows::new(offset, limit);
            match query {
                Query::Arrears => {
                    if let Some(r) = region {
                        for o in r
                            .obligations
                            .iter()
                            .flatten()
                            .filter(|o| o.arrears.value() > 0)
                        {
                            page.emit(
                                || serde_json::json!({"type":"periodic_obligation","value":o}),
                            );
                        }
                    }
                    if let Some(e) = exchange {
                        for f in e.inbox.all().filter(|f| f.fee > f.paid) {
                            page.emit(|| serde_json::json!({"type":"fact_liability","outstanding_cents":f.fee.value()-f.paid.value(),"value":f}));
                        }
                    }
                    if let Some(t) = self.engine.timeline() {
                        for c in t
                            .claims
                            .iter()
                            .filter(|c| c.processed && c.principal > c.paid)
                        {
                            page.emit(|| serde_json::json!({"type":"scheduled_claim","outstanding_cents":c.principal.value()-c.paid.value(),"value":c}));
                        }
                        for c in t.payroll.iter().filter(|c| c.billed > c.paid) {
                            page.emit(|| serde_json::json!({"type":"payroll_arrears","outstanding_cents":c.billed.value()-c.paid.value(),"value":c}));
                        }
                    }
                }
                Query::Contracts => {
                    if let Some(e) = exchange {
                        for q in &e.quotes {
                            page.emit(
                                || serde_json::json!({"type":"contract","key":q.spec.id,"value":q}),
                            );
                        }
                    }
                }
                Query::Fulfilment => {
                    if let Some(e) = exchange {
                        for ship in e.shipments.iter().flatten() {
                            page.emit(|| serde_json::json!({"type":"in_transit","quote":e.quotes[ship.quote_index].spec.id,"value":ship}));
                        }
                        for q in e
                            .quotes
                            .iter()
                            .filter(|q| q.status == mycity_core::exchange::Status::Delivered)
                        {
                            page.emit(|| serde_json::json!({"type":"physical_delivery","quote":q.spec.id,"value":q}));
                        }
                        for f in e.inbox.all() {
                            page.emit(|| serde_json::json!({"type":"fact_or_retained_claim","sequence":f.sequence,"value":f}));
                        }
                    }
                }
                Query::Capacity => {
                    for r in self.engine.resources() {
                        page.emit(|| serde_json::json!({"type":"resource","value":r}));
                    }
                    for l in self.engine.leases().iter().flatten() {
                        page.emit(|| serde_json::json!({"type":"lease","value":l}));
                    }
                    if let Some(e) = exchange {
                        page.emit(|| serde_json::json!({"type":"fact_capacity","working_used":e.inbox.facts.iter().flatten().count(),"normal_limit":mycity_core::inbox::NORMAL,"working_limit":mycity_core::inbox::TOTAL,"retained_used":e.inbox.retained.iter().flatten().count(),"retained_limit":mycity_core::inbox::RETAINED}));
                    }
                }
                Query::Blocked => {
                    // Fallible status reads are still checked even off-page.
                    let capacity = self.capacity_status()?;
                    let source = self.source_status()?;
                    page.emit(|| serde_json::json!({"type":"disk_capacity","value":capacity}));
                    page.emit(|| serde_json::json!({"type":"source_sync","value":source}));
                    if let Some(e) = exchange {
                        if e.inbox.admission_full() {
                            page.emit(|| serde_json::json!({"type":"new_commitments_blocked","reason":"normal_fact_working_capacity_exhausted"}));
                        }
                        for ship in e
                            .shipments
                            .iter()
                            .flatten()
                            .filter(|s| s.arrives_at.is_none())
                        {
                            page.emit(|| serde_json::json!({"type":"paused_transport","quote":e.quotes[ship.quote_index].spec.id,"remaining_seconds":ship.remaining,"leases_retained":ship.tokens}));
                        }
                    }
                }
                Query::Recovery => {
                    let checkpoint = self.checkpoint_status()?;
                    let source = self.source_status()?;
                    page.emit(|| serde_json::json!({"type":"checkpoint","value":checkpoint}));
                    page.emit(|| serde_json::json!({"type":"source_sync","value":source}));
                    if let Some(r) = region {
                        page.emit(|| serde_json::json!({"type":"regional_alignment","active_city":r.active_city,"interaction_time":r.interaction_time,"cities":r.cities,"snapshots":r.snapshots}));
                    }
                }
                Query::Receipts | Query::Timeline => unreachable!(),
            }
            page.finish()?
        };
        let end = offset
            .checked_add(rows.len())
            .ok_or(Error::Invalid("query_offset_overflow"))?;
        let next_cursor = if end < total {
            Some(
                String::from_utf8(json(&Cursor {
                    sequence: self.io.root.sequence,
                    generation: self.io.root.generation,
                    state: hash.clone(),
                    query,
                    offset: end,
                })?)
                .map_err(|_| Error::Invalid("query_cursor_encoding"))?,
            )
        } else {
            None
        };
        Ok(Page {
            schema: "mycity-m0-query-v1",
            sequence: self.io.root.sequence,
            timeline: self.engine.timeline().map(|t| t.progress),
            complete_boundary: self
                .engine
                .timeline()
                .is_none_or(|t| t.progress.phase.is_none()),
            state_sha256: state_hash,
            snapshot_sha256: hash,
            rows,
            next_cursor,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::cell::Cell;

    fn fixture(name: &str) -> (Store, PathBuf) {
        let parent = std::env::var_os("MYCITY_QUERY_TEST_WORK")
            .map(PathBuf::from)
            .expect("set MYCITY_QUERY_TEST_WORK to the existing external SSD evidence directory");
        let dir = parent.join(format!(
            "query-{name}-{}-{}",
            std::process::id(),
            TEMP_ID.fetch_add(1, Ordering::Relaxed)
        ));
        fs::create_dir(&dir).unwrap();
        let store = Store::create_spatial(
            &dir.join("world"),
            include_bytes!("../../../fixtures/m0-exchange-opening.json"),
            [
                (101, 1),
                (102, 1),
                (103, 1),
                (104, 1),
                (201, 2000),
                (202, 2000),
            ]
            .into_iter()
            .map(|(id, n)| Resource::new(id, n).unwrap())
            .collect(),
            "m0-storage-process-fixture-v1",
            include_bytes!("../../../fixtures/m0-exchange-spatial.json"),
            include_bytes!("../../../fixtures/terrain/m0-flat16.png"),
        )
        .unwrap();
        (store, dir)
    }

    fn apply(store: &mut Store, action: mycity_core::transaction::Action) {
        let envelope = Envelope {
            world: 1,
            branch: 1,
            operation_id: store.committed_sequence() + 1,
            expected_revision: store.engine().unwrap().world().scope_revision().1,
            actions: vec![action],
        };
        let result = store.execute(Caller::System, envelope).unwrap();
        assert!(result.model.is_ok(), "{action:?}: {:?}", result.model);
    }

    fn compare_pages(store: &mut Store) {
        for query in [
            Query::Timeline,
            Query::Contracts,
            Query::Fulfilment,
            Query::Arrears,
            Query::Capacity,
            Query::Blocked,
            Query::Recovery,
            Query::Receipts,
        ] {
            for limit in [1, 2, 3, 64] {
                let mut cursor = None;
                let mut pages = 0;
                loop {
                    let expected = store
                        .query_reference_before_page_projection(query, cursor.as_deref(), limit)
                        .unwrap();
                    let actual = store.query(query, cursor.as_deref(), limit).unwrap();
                    assert_eq!(
                        serde_json::to_value(&actual).unwrap(),
                        serde_json::to_value(&expected).unwrap(),
                        "{query:?}/{limit}/{pages}"
                    );
                    assert!(actual.rows.len() <= limit);
                    cursor = actual.next_cursor;
                    pages += 1;
                    assert!(pages < 100);
                    if cursor.is_none() {
                        break;
                    }
                }
            }
        }
    }

    #[test]
    fn actual_store_pages_match_frozen_v172_oracle_before_and_after_recovery() {
        use mycity_core::{exchange::Command as X, inbox::Command as I, transaction::Action};
        let (mut store, dir) = fixture("reference");
        apply(&mut store, Action::Inbox { command: I::Enable });
        for quote in 1..=4 {
            apply(
                &mut store,
                Action::Exchange {
                    command: X::Dispatch { quote },
                },
            );
        }
        apply(
            &mut store,
            Action::Exchange {
                command: X::SetLink {
                    edge: 1,
                    open: false,
                },
            },
        );
        for quote in 1..=4 {
            apply(
                &mut store,
                Action::Inbox {
                    command: I::Incident { quote },
                },
            );
        }
        let before_hash = store.state_hash().unwrap();
        let before_sequence = store.committed_sequence();
        compare_pages(&mut store);
        assert_eq!(store.state_hash().unwrap(), before_hash);
        assert_eq!(store.committed_sequence(), before_sequence);
        let cursor = store
            .query(Query::Contracts, None, 1)
            .unwrap()
            .next_cursor
            .unwrap();
        store.checkpoint().unwrap();
        assert!(matches!(
            store.query(Query::Contracts, Some(&cursor), 1),
            Err(Error::Invalid("query_snapshot_changed"))
        ));
        drop(store);
        let mut store = Store::open(&dir.join("world"), "m0-storage-process-fixture-v1").unwrap();
        assert_eq!(store.state_hash().unwrap(), before_hash);
        compare_pages(&mut store);
        apply(
            &mut store,
            Action::Exchange {
                command: X::SetLink {
                    edge: 1,
                    open: true,
                },
            },
        );
        apply(&mut store, Action::RegionAdvance { seconds: 120 });
        let delivered = store.query(Query::Fulfilment, None, 64).unwrap();
        assert!(delivered
            .rows
            .iter()
            .any(|r| r["type"] == "physical_delivery"));
        compare_pages(&mut store);
        drop(store);
        // The previous world intentionally has a full inbox. Do not weaken its
        // admission gate to set up a different Timeline pagination scenario.
        let mut store = Store::create(
            &dir.join("timeline-world"),
            include_bytes!("../../../fixtures/m0-e0-opening.json"),
            vec![],
            "m0-storage-process-fixture-v1",
        )
        .unwrap();
        use mycity_core::{
            timeline::{Command as T, Cycle, PayrollSpec},
            units::Money,
        };
        apply(&mut store, Action::Timeline { command: T::Enable });
        apply(
            &mut store,
            Action::Timeline {
                command: T::Claims {
                    first_id: 1,
                    count: 3,
                    payer: 3,
                    payee: 2,
                    amount: Money::new(1).unwrap(),
                    due: 3600,
                },
            },
        );
        apply(
            &mut store,
            Action::Timeline {
                command: T::AddPayroll {
                    spec: PayrollSpec {
                        id: 1,
                        payer: 3,
                        payee: 2,
                        home_city: 0,
                        hourly_cents: Money::new(1).unwrap(),
                        cycle: Cycle::Daily,
                        ends_at: 86400,
                    },
                },
            },
        );
        let timeline = store.query(Query::Timeline, None, 64).unwrap();
        assert_eq!(timeline.rows.len(), 5);
        assert_eq!(timeline.rows[4]["type"], "payroll");
        compare_pages(&mut store);
        // Retain complete successful/failed worlds for the root validation evidence.
    }

    #[test]
    fn actual_offsets_limits_and_off_page_source_errors_are_not_hidden() {
        let (mut store, dir) = fixture("errors");
        let page = store.query(Query::Contracts, None, 64).unwrap();
        let count = page.rows.len();
        let generation = store.io.root.generation;
        let cursor = |offset| {
            String::from_utf8(
                json(&Cursor {
                    sequence: page.sequence,
                    generation,
                    state: page.snapshot_sha256.clone(),
                    query: Query::Contracts,
                    offset,
                })
                .unwrap(),
            )
            .unwrap()
        };
        let timeline_cursor = String::from_utf8(
            json(&Cursor {
                sequence: page.sequence,
                generation,
                state: page.snapshot_sha256.clone(),
                query: Query::Timeline,
                offset: 1,
            })
            .unwrap(),
        )
        .unwrap();
        // Preserve the existing disabled-Timeline empty-page behavior.
        let expected = store
            .query_reference_before_page_projection(Query::Timeline, Some(&timeline_cursor), 1)
            .unwrap();
        let actual = store
            .query(Query::Timeline, Some(&timeline_cursor), 1)
            .unwrap();
        assert!(actual.rows.is_empty());
        assert_eq!(
            serde_json::to_value(actual).unwrap(),
            serde_json::to_value(expected).unwrap()
        );
        let end = cursor(count);
        let beyond = cursor(count + 1);
        assert!(store
            .query(Query::Contracts, Some(&end), 1)
            .unwrap()
            .rows
            .is_empty());
        assert!(matches!(
            store.query(Query::Contracts, Some(&beyond), 1),
            Err(Error::Invalid("query_offset"))
        ));
        for limit in [0, 65, usize::MAX] {
            assert!(matches!(
                store.query(Query::Contracts, None, limit),
                Err(Error::Invalid("query_page_limit"))
            ));
        }
        fs::write(dir.join("world/producer-outbox.json"), b"broken").unwrap();
        // A one-row page excludes source_sync. The snapshot-level source read
        // must still fail; branch-local error ordering is separately source-reviewed.
        assert!(matches!(
            store.query(Query::Recovery, None, 1),
            Err(Error::Invalid("invalid_json"))
        ));
        assert!(matches!(
            store.query(Query::Blocked, None, 1),
            Err(Error::Invalid("invalid_json"))
        ));
    }

    #[test]
    fn skipped_rows_never_project_or_grow_the_output_vector() {
        for (offset, limit) in [(0, 1), (998, 2), (999, 64), (1000, 64)] {
            let projections = Cell::new(0);
            let mut page = PageRows::new(offset, limit);
            let capacity = page.rows.capacity();
            for id in 0..1000 {
                page.emit(|| {
                    projections.set(projections.get() + 1);
                    serde_json::json!({"id":id})
                });
            }
            assert_eq!(page.rows.capacity(), capacity);
            let (rows, total) = page.finish().unwrap();
            assert_eq!(total, 1000);
            assert_eq!(rows.len(), limit.min(1000 - offset));
            assert_eq!(projections.get(), rows.len());
            for (index, row) in rows.iter().enumerate() {
                assert_eq!(row["id"], offset + index);
            }
        }
    }

    #[test]
    fn beyond_end_offset_rejects_without_projecting() {
        let mut page = PageRows::new(usize::MAX, 64);
        page.emit(|| panic!("off-page projection"));
        assert!(matches!(page.finish(), Err(Error::Invalid("query_offset"))));
    }
}

#[cfg(test)]
#[path = "query_reference_before_page_projection.rs"]
mod reference_before_page_projection;
