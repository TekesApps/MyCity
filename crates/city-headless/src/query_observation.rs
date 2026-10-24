//! Local operator observations from Store query records and diagnostic status.
//! Authorization and transport belong to the caller. This module grants neither.
use mycity_storage::{Error, Page, Query, Store};
use serde::Serialize;

#[derive(Debug, PartialEq, Eq, Serialize)]
#[serde(tag = "status", rename_all = "snake_case")]
pub(crate) enum Evidence<T> {
    Known { value: T },
    Unknown { reason: UnknownReason },
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub(crate) enum UnknownReason {
    NoGeneralObservationTimeProvider,
    NoConditionalEtaProvider,
    NoGeneralBlockReasonProvider,
    NoRegionalSnapshotProvider,
    SnapshotAheadOfInteractionClock,
}

#[derive(Debug, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub(crate) enum RecordMeaning {
    /// Query output may be a business record or live storage/source diagnostic.
    /// Neither establishes a prediction or business success.
    StoreQueryRecordOrDiagnosticStatusNotPredictionOrBusinessSuccess,
}

#[derive(Debug, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub(crate) enum BlockReason {
    /// The raw row establishes a paused transport, not why its link is paused.
    TransportPausedCauseUnspecified,
    NormalFactWorkingCapacityExhausted,
}

#[derive(Debug, Serialize)]
pub(crate) struct RowObservation {
    /// Index in this page only; identity and values remain in page.rows[index].
    pub row_index: usize,
    pub actual_or_forecast: Evidence<RecordMeaning>,
    pub observed_at: Evidence<u64>,
    pub age_seconds: Evidence<u64>,
    pub block_reason: Evidence<BlockReason>,
    /// Transport remaining_seconds is work remaining, never a conditional ETA.
    pub conditional_eta: Evidence<u64>,
}

#[derive(Debug, Serialize)]
pub(crate) struct RegionalSnapshotObservation {
    pub city: u32,
    pub version: u64,
    /// Population snapshot provenance only, not observation time for query rows.
    pub observed_at: u64,
    pub applied_sequence: u64,
    pub age_seconds: Evidence<u64>,
}

#[derive(Debug, Serialize)]
pub(crate) struct RegionalObservations {
    pub source: Evidence<&'static str>,
    pub coverage: Evidence<&'static str>,
    pub interaction_time: u64,
    pub snapshots: Vec<RegionalSnapshotObservation>,
}

#[derive(Debug, Serialize)]
pub(crate) struct ObservationPage {
    pub schema: &'static str,
    pub purpose: &'static str,
    pub source: Evidence<&'static str>,
    /// Wire schema of this exact Page, not a model/rules or provider version.
    pub source_version: Evidence<&'static str>,
    pub section: Query,
    pub coverage: Evidence<&'static str>,
    /// Moved from Store::query; all hashes, rows, cursor and progress are original.
    pub page: Page,
    pub row_observations: Vec<RowObservation>,
    pub regional_snapshots: Evidence<RegionalObservations>,
}

pub(crate) fn observe(
    store: &mut Store,
    query: Query,
    cursor: Option<&str>,
    limit: usize,
) -> Result<ObservationPage, Error> {
    // One authority supplies pagination and all fallible source/status checks.
    // Do not duplicate its limit validation, cursor parsing or row projections.
    let page = store.query(query, cursor, limit)?;
    let row_observations = page.rows.iter().enumerate().map(|(row_index, row)| {
        let block_reason = match row.get("type").and_then(serde_json::Value::as_str) {
            Some("paused_transport") => Evidence::Known {
                value: BlockReason::TransportPausedCauseUnspecified,
            },
            Some("new_commitments_blocked")
                if row.get("reason").and_then(serde_json::Value::as_str)
                    == Some("normal_fact_working_capacity_exhausted") => Evidence::Known {
                value: BlockReason::NormalFactWorkingCapacityExhausted,
            },
            _ => Evidence::Unknown { reason: UnknownReason::NoGeneralBlockReasonProvider },
        };
        RowObservation {
            row_index,
            actual_or_forecast: Evidence::Known {
                value: RecordMeaning::StoreQueryRecordOrDiagnosticStatusNotPredictionOrBusinessSuccess,
            },
            observed_at: Evidence::Unknown { reason: UnknownReason::NoGeneralObservationTimeProvider },
            age_seconds: Evidence::Unknown { reason: UnknownReason::NoGeneralObservationTimeProvider },
            block_reason,
            conditional_eta: Evidence::Unknown { reason: UnknownReason::NoConditionalEtaProvider },
        }
    }).collect();
    // The exclusive &mut Store has not been released and no mutation, reopen,
    // external status read or second query occurs between Page and this borrow.
    let regional_snapshots = match store.engine()?.world().query_region() {
        Some(region) => Evidence::Known {
            value: RegionalObservations {
                source: Evidence::Known { value: "same_store.region.snapshots" },
                coverage: Evidence::Known { value: "regional_population_snapshots_only_not_query_row_freshness" },
                interaction_time: region.interaction_time,
                snapshots: region.snapshots.iter().flatten().map(|snapshot| {
                    RegionalSnapshotObservation {
                        city: snapshot.city,
                        version: snapshot.version,
                        observed_at: snapshot.observed_at,
                        applied_sequence: snapshot.applied_sequence,
                        age_seconds: match region.interaction_time.checked_sub(snapshot.observed_at) {
                            Some(age) => Evidence::Known { value: age },
                            None => Evidence::Unknown { reason: UnknownReason::SnapshotAheadOfInteractionClock },
                        },
                    }
                }).collect(),
            },
        },
        None => Evidence::Unknown { reason: UnknownReason::NoRegionalSnapshotProvider },
    };
    Ok(ObservationPage {
        schema: "mycity-operator-observation-v1",
        purpose: "local_operator_diagnostics",
        source: Evidence::Known { value: "store.query" },
        source_version: Evidence::Known { value: page.schema },
        section: query,
        coverage: Evidence::Known { value: "requested_section_current_page_only_no_general_forecast_goal_monitor_provider" },
        page,
        row_observations,
        regional_snapshots,
    })
}

#[cfg(test)]
#[path = "query_observation_tests.rs"]
mod tests;
