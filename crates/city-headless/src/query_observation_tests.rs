//! Source-only tests using legal experimental Stores, not production providers.
use super::*;
use mycity_core::{
    exchange::Command as X,
    inbox::Command as I,
    timeline::Command as T,
    transaction::{Action, Caller, Envelope, Resource},
    units::Money,
};
use std::{fs, path::PathBuf, sync::atomic::{AtomicU64, Ordering}};

static ID: AtomicU64 = AtomicU64::new(0);
const RULES: &str = "m0-storage-process-fixture-v1";

fn directory(name: &str) -> PathBuf {
    let parent = PathBuf::from(std::env::var_os("MYCITY_OBSERVATION_TEST_WORK")
        .expect("set MYCITY_OBSERVATION_TEST_WORK to an existing external SSD evidence directory"));
    assert!(parent.is_dir(), "evidence parent must already exist");
    let dir = parent.join(format!("observation-{name}-{}-{}", std::process::id(), ID.fetch_add(1, Ordering::Relaxed)));
    fs::create_dir(&dir).unwrap();
    // Retain successful and failed worlds for root evidence; no Drop cleanup.
    dir
}

fn exchange(name: &str) -> (Store, PathBuf) {
    let dir = directory(name);
    let store = Store::create_spatial(
        &dir.join("world"),
        include_bytes!("../../../fixtures/m0-exchange-opening.json"),
        [(101, 1), (102, 1), (103, 1), (104, 1), (201, 2000), (202, 2000)]
            .into_iter().map(|(id, n)| Resource::new(id, n).unwrap()).collect(),
        RULES,
        include_bytes!("../../../fixtures/m0-exchange-spatial.json"),
        include_bytes!("../../../fixtures/terrain/m0-flat16.png"),
    ).unwrap();
    (store, dir)
}

fn apply(store: &mut Store, action: Action) {
    let envelope = Envelope {
        world: 1,
        branch: 1,
        operation_id: store.committed_sequence() + 1,
        expected_revision: store.engine().unwrap().world().scope_revision().1,
        actions: vec![action],
    };
    let result = store.execute(Caller::System, envelope).unwrap();
    assert!(result.model.is_ok(), "{action:?}: {:?}", result.model);
    assert!(result.has_persistent_receipt);
}

fn compare_all(store: &mut Store) {
    let before_hash = store.state_hash().unwrap();
    let before_sequence = store.committed_sequence();
    for query in [Query::Timeline, Query::Contracts, Query::Fulfilment, Query::Arrears,
        Query::Capacity, Query::Blocked, Query::Recovery, Query::Receipts] {
        for limit in [1, 2, 64] {
            let mut cursor = None;
            for index in 0..100 {
                let expected = store.query(query, cursor.as_deref(), limit).unwrap();
                let observed = observe(store, query, cursor.as_deref(), limit).unwrap();
                assert_eq!(serde_json::to_value(&observed.page).unwrap(), serde_json::to_value(&expected).unwrap());
                assert_eq!(observed.schema, "mycity-operator-observation-v1");
                assert_eq!(observed.source, Evidence::Known { value: "store.query" });
                assert_eq!(observed.source_version, Evidence::Known { value: expected.schema });
                assert_eq!(observed.coverage, Evidence::Known {
                    value: "requested_section_current_page_only_no_general_forecast_goal_monitor_provider",
                });
                assert_eq!(observed.section, query);
                assert_eq!(observed.row_observations.len(), expected.rows.len());
                assert!(observed.row_observations.len() <= 64);
                for (row_index, metadata) in observed.row_observations.iter().enumerate() {
                    assert_eq!(metadata.row_index, row_index);
                    assert_eq!(metadata.actual_or_forecast, Evidence::Known {
                        value: RecordMeaning::StoreQueryRecordOrDiagnosticStatusNotPredictionOrBusinessSuccess,
                    });
                    assert_eq!(metadata.observed_at, Evidence::Unknown { reason: UnknownReason::NoGeneralObservationTimeProvider });
                    assert_eq!(metadata.age_seconds, Evidence::Unknown { reason: UnknownReason::NoGeneralObservationTimeProvider });
                    assert_eq!(metadata.conditional_eta, Evidence::Unknown { reason: UnknownReason::NoConditionalEtaProvider });
                    // No duplicate rawValue/hash/sequence is hidden in row metadata.
                    let value = serde_json::to_value(metadata).unwrap();
                    assert_eq!(value.as_object().unwrap().len(), 6);
                    match expected.rows[row_index]["type"].as_str() {
                        Some("paused_transport") => assert_eq!(metadata.block_reason, Evidence::Known {
                            value: BlockReason::TransportPausedCauseUnspecified,
                        }),
                        Some("new_commitments_blocked") => assert_eq!(metadata.block_reason, Evidence::Known {
                            value: BlockReason::NormalFactWorkingCapacityExhausted,
                        }),
                        _ => assert_eq!(metadata.block_reason, Evidence::Unknown {
                            reason: UnknownReason::NoGeneralBlockReasonProvider,
                        }),
                    }
                }
                cursor = expected.next_cursor;
                if cursor.is_none() { break; }
                assert!(index < 99, "fixture pagination must terminate");
            }
        }
    }
    assert_eq!(store.state_hash().unwrap(), before_hash);
    assert_eq!(store.committed_sequence(), before_sequence);
}

#[test]
fn actual_store_all_sections_pages_and_reopen_preserve_raw_page() {
    let (mut store, dir) = exchange("raw");
    apply(&mut store, Action::Inbox { command: I::Enable });
    for quote in 1..=4 { apply(&mut store, Action::Exchange { command: X::Dispatch { quote } }); }
    apply(&mut store, Action::Exchange { command: X::SetLink { edge: 1, open: false } });
    for quote in 1..=4 { apply(&mut store, Action::Inbox { command: I::Incident { quote } }); }
    compare_all(&mut store);
    store.checkpoint().unwrap();
    drop(store);
    let mut store = Store::open(&dir.join("world"), RULES).unwrap();
    compare_all(&mut store);
    apply(&mut store, Action::Exchange { command: X::SetLink { edge: 1, open: true } });
    apply(&mut store, Action::RegionAdvance { seconds: 120 });
    assert!(observe(&mut store, Query::Fulfilment, None, 64).unwrap().page.rows.iter()
        .any(|r| r["type"] == "physical_delivery"));
    compare_all(&mut store);
}

#[test]
fn actual_timeline_processing_is_not_promoted_to_last_complete() {
    // The existing quote reserves 20 due claims + 86 fixed steps: 106 total.
    // Width 1 exceeds the 96-page gate; width 2 is the minimum legal width.
    // Keep that width throughout the existing persisted continuation ticket.
    const PAGE_LIMIT: u16 = 2;
    let dir = directory("timeline");
    let mut store = Store::create(&dir.join("world"), include_bytes!("../../../fixtures/m0-e0-opening.json"), vec![], RULES).unwrap();
    apply(&mut store, Action::Timeline { command: T::Enable });
    apply(&mut store, Action::Timeline { command: T::Claims {
        first_id: 1, count: 20, payer: 3, payee: 2, amount: Money::new(1).unwrap(), due: 3600,
    } });
    apply(&mut store, Action::Timeline { command: T::Advance { target: 3600, limit: PAGE_LIMIT } });
    let observed = observe(&mut store, Query::Timeline, None, 2).unwrap();
    let progress = observed.page.timeline.unwrap();
    assert_eq!(progress.target, Some(3600));
    assert!(progress.phase.is_some());
    assert!(!observed.page.complete_boundary);
    assert!(progress.last_complete < progress.at);
    assert_eq!(store.continuation_status().unwrap().unwrap().page_limit, PAGE_LIMIT);
    assert_eq!(observed.page.rows[0]["progress"], serde_json::to_value(progress).unwrap());
    assert!(matches!(observed.regional_snapshots, Evidence::Unknown { reason: UnknownReason::NoRegionalSnapshotProvider }));
    compare_all(&mut store);
    let mut completed = false;
    for _ in 0..100 {
        apply(&mut store, Action::Timeline { command: T::Advance { target: 3600, limit: PAGE_LIMIT } });
        let observed = observe(&mut store, Query::Timeline, None, 2).unwrap();
        let progress = observed.page.timeline.unwrap();
        if progress.target.is_none() {
            assert_eq!(progress.last_complete, 3600);
            assert!(observed.page.complete_boundary);
            completed = true;
            break;
        }
    }
    assert!(completed);
    let claims = &store.engine().unwrap().timeline().unwrap().claims;
    assert_eq!(claims.len(), 20);
    assert!(claims.iter().all(|claim| claim.processed && claim.paid == Money::new(1).unwrap()));
    compare_all(&mut store);
}

#[test]
fn actual_snapshot_provenance_is_same_store_and_has_narrow_coverage() {
    let (mut store, _) = exchange("provenance");
    apply(&mut store, Action::RegionAdvance { seconds: 120 });
    apply(&mut store, Action::RegionActivate { city: 2 });
    let observed = observe(&mut store, Query::Recovery, None, 64).unwrap();
    let region = store.engine().unwrap().world().query_region().unwrap();
    let Evidence::Known { value } = observed.regional_snapshots else { panic!("region exists") };
    assert_eq!(value.interaction_time, region.interaction_time);
    assert_eq!(value.source, Evidence::Known { value: "same_store.region.snapshots" });
    assert_eq!(value.coverage, Evidence::Known { value: "regional_population_snapshots_only_not_query_row_freshness" });
    assert_eq!(value.snapshots.len(), region.snapshots.iter().flatten().count());
    assert!(value.snapshots.len() <= mycity_core::region::MAX_CITIES);
    for (actual, expected) in value.snapshots.iter().zip(region.snapshots.iter().flatten()) {
        assert_eq!((actual.city, actual.version, actual.observed_at, actual.applied_sequence),
            (expected.city, expected.version, expected.observed_at, expected.applied_sequence));
        assert_eq!(actual.age_seconds, Evidence::Known { value: region.interaction_time - expected.observed_at });
    }
    assert!(observed.row_observations.iter().all(|row| matches!(row.observed_at, Evidence::Unknown { .. })));
}

#[test]
fn actual_paused_transport_remaining_is_not_eta_or_delivery() {
    let (mut store, _) = exchange("paused");
    apply(&mut store, Action::Exchange { command: X::Dispatch { quote: 1 } });
    apply(&mut store, Action::Exchange { command: X::SetLink { edge: 1, open: false } });
    let observed = observe(&mut store, Query::Blocked, None, 64).unwrap();
    let index = observed.page.rows.iter().position(|r| r["type"] == "paused_transport").unwrap();
    assert!(observed.page.rows[index]["remaining_seconds"].as_u64().unwrap() > 0);
    let metadata = &observed.row_observations[index];
    assert_eq!(metadata.block_reason, Evidence::Known { value: BlockReason::TransportPausedCauseUnspecified });
    assert_eq!(metadata.conditional_eta, Evidence::Unknown { reason: UnknownReason::NoConditionalEtaProvider });
    assert_eq!(metadata.actual_or_forecast, Evidence::Known { value: RecordMeaning::StoreQueryRecordOrDiagnosticStatusNotPredictionOrBusinessSuccess });
}

#[test]
fn actual_query_errors_limits_and_old_cursors_pass_through() {
    let (mut store, dir) = exchange("errors");
    for limit in [0, 65, usize::MAX] {
        assert!(matches!(store.query(Query::Contracts, None, limit), Err(Error::Invalid("query_page_limit"))));
        assert!(matches!(observe(&mut store, Query::Contracts, None, limit), Err(Error::Invalid("query_page_limit"))));
    }
    for cursor in ["broken".to_owned(), "x".repeat(1025)] {
        let raw = store.query(Query::Contracts, Some(&cursor), 1).unwrap_err();
        let wrapped = observe(&mut store, Query::Contracts, Some(&cursor), 1).unwrap_err();
        assert_eq!(format!("{raw:?}"), format!("{wrapped:?}"));
    }
    let cursor = observe(&mut store, Query::Contracts, None, 1).unwrap().page.next_cursor.unwrap();
    let mut beyond: serde_json::Value = serde_json::from_str(&cursor).unwrap();
    beyond["offset"] = serde_json::json!(usize::MAX);
    let beyond = serde_json::to_string(&beyond).unwrap();
    assert!(matches!(store.query(Query::Contracts, Some(&beyond), 1), Err(Error::Invalid("query_offset"))));
    assert!(matches!(observe(&mut store, Query::Contracts, Some(&beyond), 1), Err(Error::Invalid("query_offset"))));
    assert!(matches!(observe(&mut store, Query::Capacity, Some(&cursor), 1), Err(Error::Invalid("query_snapshot_changed"))));
    store.checkpoint().unwrap();
    assert!(matches!(observe(&mut store, Query::Contracts, Some(&cursor), 1), Err(Error::Invalid("query_snapshot_changed"))));
    let cursor = observe(&mut store, Query::Contracts, None, 1).unwrap().page.next_cursor.unwrap();
    apply(&mut store, Action::RegionAdvance { seconds: 1 });
    assert!(matches!(observe(&mut store, Query::Contracts, Some(&cursor), 1), Err(Error::Invalid("query_snapshot_changed"))));
    fs::write(dir.join("world/producer-outbox.json"), b"broken").unwrap();
    assert!(matches!(store.query(Query::Recovery, None, 1), Err(Error::Invalid("invalid_json"))));
    assert!(matches!(observe(&mut store, Query::Recovery, None, 1), Err(Error::Invalid("invalid_json"))));
}
