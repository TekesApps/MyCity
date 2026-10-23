//! Bounded durable records for household relocation. Monetary debt remains in Timeline Claims.
use crate::{
    housing::{MoveWait, OpeningHousing},
    timeline::{Claim, RentClaimSource},
    units::GameTime,
};
use serde::{
    de::{self, SeqAccess, Visitor},
    Deserialize, Deserializer, Serialize,
};
use std::fmt;
use std::mem::size_of;

pub const MAX_HISTORICAL_LEASES: usize = 32;
pub const MAX_MOVE_WAITS: usize = 32;

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct HistoricalLease {
    pub city_id: u32,
    pub household_id: u32,
    pub unit_id: u32,
    pub lease_id: u32,
    pub closed_at: GameTime,
    pub claim_id: u64,
    pub source: RentClaimSource,
}
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct MoveWaitRecord {
    pub operation_id: u64,
    pub household_id: u32,
    pub city_id: u32,
    pub assessment_id: u64,
    pub assessment_version: u64,
    pub observed_at: GameTime,
    pub reason: MoveWait,
}
#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct MoveRuntimeState {
    // Backing is allocated only for active rows and remains capped at the original 32-row
    // limits. A wait-only checkpoint must not reserve an unrelated empty history table.
    #[serde(default, deserialize_with = "deserialize_history")]
    historical_leases: Vec<HistoricalLease>,
    #[serde(default, deserialize_with = "deserialize_waits")]
    waits: Vec<MoveWaitRecord>,
}
fn deserialize_bounded<'de, D, T, const MAX: usize>(deserializer: D) -> Result<Vec<T>, D::Error>
where
    D: Deserializer<'de>,
    T: Deserialize<'de>,
{
    struct BoundedVisitor<T, const MAX: usize>(std::marker::PhantomData<T>);
    impl<'de, T: Deserialize<'de>, const MAX: usize> Visitor<'de> for BoundedVisitor<T, MAX> {
        type Value = Vec<T>;

        fn expecting(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
            write!(formatter, "at most {MAX} bounded housing move rows")
        }

        fn visit_seq<A: SeqAccess<'de>>(self, mut seq: A) -> Result<Self::Value, A::Error> {
            if seq.size_hint().is_some_and(|hint| hint > MAX) {
                return Err(de::Error::invalid_length(MAX.saturating_add(1), &self));
            }
            let mut rows = Vec::new();
            loop {
                if rows.len() == MAX {
                    if seq.next_element::<de::IgnoredAny>()?.is_some() {
                        return Err(de::Error::invalid_length(MAX.saturating_add(1), &self));
                    }
                    return Ok(rows);
                }
                let Some(row) = seq.next_element()? else {
                    break;
                };
                rows.try_reserve_exact(1)
                    .map_err(|_| de::Error::custom("housing_move_allocation"))?;
                if rows.capacity() > MAX {
                    return Err(de::Error::custom("housing_move_capacity"));
                }
                rows.push(row);
            }
            Ok(rows)
        }
    }
    deserializer.deserialize_seq(BoundedVisitor::<T, MAX>(std::marker::PhantomData))
}
fn deserialize_history<'de, D: Deserializer<'de>>(
    deserializer: D,
) -> Result<Vec<HistoricalLease>, D::Error> {
    deserialize_bounded::<D, _, MAX_HISTORICAL_LEASES>(deserializer)
}
fn deserialize_waits<'de, D: Deserializer<'de>>(
    deserializer: D,
) -> Result<Vec<MoveWaitRecord>, D::Error> {
    deserialize_bounded::<D, _, MAX_MOVE_WAITS>(deserializer)
}

impl MoveRuntimeState {
    pub fn history(&self) -> &[HistoricalLease] {
        &self.historical_leases
    }
    pub fn waits(&self) -> &[MoveWaitRecord] {
        &self.waits
    }
    pub fn owned_capacity_bytes(&self) -> usize {
        size_of::<Self>()
            + self.historical_leases.capacity() * size_of::<HistoricalLease>()
            + self.waits.capacity() * size_of::<MoveWaitRecord>()
    }
    pub fn is_empty(&self) -> bool {
        self.history().is_empty() && self.waits().is_empty()
    }
    pub fn wait_for(&self, household_id: u32) -> Option<MoveWaitRecord> {
        self.waits
            .iter()
            .find(|row| row.household_id == household_id)
            .copied()
    }
    pub fn historical_source(&self, claim: &Claim) -> Option<RentClaimSource> {
        self.historical_leases
            .iter()
            .find(|row| row.claim_id == claim.id && row.source.lease_id == row.lease_id)
            .map(|row| row.source)
    }
    pub fn validate(
        &self,
        opening: &OpeningHousing,
        city_id: Option<u32>,
    ) -> Result<(), &'static str> {
        if self.history().len() > MAX_HISTORICAL_LEASES
            || self.waits().len() > MAX_MOVE_WAITS
            || self.historical_leases.capacity() > MAX_HISTORICAL_LEASES
            || self.waits.capacity() > MAX_MOVE_WAITS
        {
            return Err("housing_move_limit");
        }
        if self
            .history()
            .windows(2)
            .any(|rows| rows[0].claim_id >= rows[1].claim_id)
            || self
                .waits()
                .windows(2)
                .any(|rows| rows[0].household_id >= rows[1].household_id)
        {
            return Err("housing_move_order");
        }
        for (i, h) in self.history().iter().enumerate() {
            if h.city_id == 0
                || Some(h.city_id) != city_id
                || h.household_id == 0
                || h.unit_id == 0
                || h.lease_id == 0
                || h.claim_id != u64::from(h.lease_id)
                || h.source.lease_id != h.lease_id
                || h.source.unit_id != h.unit_id
                || h.source.tenant == 0
                || h.source.landlord == 0
                || opening.household(h.household_id).is_err()
                || self.history()[..i].iter().any(|p| {
                    p.claim_id == h.claim_id
                        || (p.household_id == h.household_id
                            && p.unit_id == h.unit_id
                            && p.lease_id == h.lease_id)
                })
            {
                return Err("housing_move_history");
            }
        }
        for (i, w) in self.waits().iter().enumerate() {
            if w.operation_id == 0
                || w.household_id == 0
                || Some(w.city_id) != city_id
                || w.assessment_id == 0
                || w.assessment_version == 0
                || opening.household(w.household_id).is_err()
                || opening
                    .move_assessments
                    .binary_search_by_key(&w.assessment_id, |a| a.assessment_id)
                    .ok()
                    .is_none_or(|index| {
                        opening.move_assessments[index].version != w.assessment_version
                    })
                || self.waits()[..i]
                    .iter()
                    .any(|p| p.household_id == w.household_id)
            {
                return Err("housing_move_wait");
            }
        }
        Ok(())
    }
    pub fn validate_timeline(
        &self,
        opening: &OpeningHousing,
        state: &crate::housing::HousingState,
        timeline: &crate::timeline::Timeline,
    ) -> Result<(), &'static str> {
        self.validate(opening, opening.city_id)?;
        for unit in state.units().iter().filter(|unit| unit.lease_id != 0) {
            let claim_id = u64::from(unit.lease_id);
            let claim = timeline
                .claims
                .binary_search_by_key(&claim_id, |claim| claim.id)
                .ok()
                .map(|index| &timeline.claims[index])
                .ok_or("housing_move_active_claim")?;
            let source = timeline
                .rent_source_for_claim(claim)
                .ok_or("housing_move_active_claim")?;
            if source.unit_id != unit.unit_id
                || source.tenant
                    != opening
                        .households
                        .iter()
                        .find(|row| row.household_id == unit.household_id)
                        .map(|row| row.account)
                        .unwrap_or(0)
                || source.landlord != unit.owner_account
                || claim.payer != source.tenant
                || claim.payee != source.landlord
                || claim.source_ref == 0
                || self
                    .history()
                    .iter()
                    .any(|history| history.claim_id == claim_id)
            {
                return Err("housing_move_active_claim");
            }
        }
        for history in self.history() {
            let claim = timeline
                .claims
                .iter()
                .find(|claim| claim.id == history.claim_id)
                .ok_or("housing_move_history_claim")?;
            if timeline.rent_source_for_claim(claim) != Some(history.source)
                || claim.payer != history.source.tenant
                || claim.payee != history.source.landlord
                || claim.principal < claim.paid
                || claim.source_ref == 0
                || history.closed_at
                    > GameTime::new(
                        i64::try_from(timeline.progress.last_complete)
                            .map_err(|_| "housing_move_history_time")?,
                    )
                    .map_err(|_| "housing_move_history_time")?
            {
                return Err("housing_move_history_claim");
            }
        }
        // Every retained rent source belongs to exactly one current lease or one historical
        // lease. Extra orphan provenance cannot silently retain or manufacture occupancy.
        for claim in &timeline.claims {
            let Some(source) = timeline.rent_source_for_claim(claim) else {
                continue;
            };
            let active = state.units().iter().any(|unit| {
                unit.lease_id == source.lease_id
                    && unit.unit_id == source.unit_id
                    && unit.owner_account == source.landlord
                    && opening.households.iter().any(|household| {
                        household.household_id == unit.household_id
                            && household.account == source.tenant
                    })
            });
            let historical = self.history().iter().any(|history| {
                history.claim_id == claim.id
                    && history.source == source
                    && history.lease_id == source.lease_id
                    && history.unit_id == source.unit_id
            });
            if active == historical {
                return Err("housing_move_orphan_rent_source");
            }
        }
        Ok(())
    }
    pub fn record_wait(
        &mut self,
        record: MoveWaitRecord,
        expected_operation_id: u64,
    ) -> Result<(), &'static str> {
        let found = self
            .waits
            .binary_search_by_key(&record.household_id, |w| w.household_id);
        match found {
            Ok(index)
                if self.waits[index].operation_id == expected_operation_id
                    && expected_operation_id != 0 =>
            {
                self.waits[index] = record
            }
            Ok(_) => return Err("housing_move_wait_cas"),
            Err(index) if expected_operation_id == 0 && self.waits.len() < MAX_MOVE_WAITS => {
                self.waits
                    .try_reserve_exact(1)
                    .map_err(|_| "housing_move_wait_full")?;
                if self.waits.capacity() > MAX_MOVE_WAITS {
                    return Err("housing_move_wait_full");
                }
                self.waits.insert(index, record);
            }
            Err(_) => return Err("housing_move_wait_full"),
        }
        Ok(())
    }
    pub fn clear_wait(
        &mut self,
        household_id: u32,
        expected_operation_id: u64,
    ) -> Result<(), &'static str> {
        let index = self
            .waits
            .binary_search_by_key(&household_id, |w| w.household_id)
            .map_err(|_| "housing_move_wait_cas")?;
        if self.waits[index].operation_id != expected_operation_id || expected_operation_id == 0 {
            return Err("housing_move_wait_cas");
        }
        self.waits.remove(index);
        if self.waits.is_empty() {
            self.waits = Vec::new();
        }
        Ok(())
    }
    pub fn append_history(&mut self, record: HistoricalLease) -> Result<(), &'static str> {
        if self.history().len() >= MAX_HISTORICAL_LEASES {
            return Err("housing_move_history_full");
        }
        if self.history().iter().any(|h| {
            h.claim_id == record.claim_id
                || (h.household_id == record.household_id && h.lease_id == record.lease_id)
        }) || self
            .history()
            .last()
            .is_some_and(|last| last.claim_id >= record.claim_id)
        {
            return Err("housing_move_history_duplicate");
        }
        self.historical_leases
            .try_reserve_exact(1)
            .map_err(|_| "housing_move_history_full")?;
        if self.historical_leases.capacity() > MAX_HISTORICAL_LEASES {
            return Err("housing_move_history_full");
        }
        self.historical_leases.push(record);
        Ok(())
    }
}

#[cfg(test)]
mod layout_tests {
    use super::*;
    use crate::housing::{OpeningMoveAssessment, OpeningMoveCandidate, MAX_M0_HOUSING_UNITS};

    #[test]
    fn bounded_move_storage_layout_report() {
        let mut state = MoveRuntimeState::default();
        assert_eq!(state.owned_capacity_bytes(), size_of::<MoveRuntimeState>());
        assert_eq!(state.historical_leases.capacity(), 0);
        assert_eq!(state.waits.capacity(), 0);
        let history_row = HistoricalLease {
            city_id: 1,
            household_id: 10,
            unit_id: 100,
            lease_id: 200,
            closed_at: GameTime::ZERO,
            claim_id: 200,
            source: RentClaimSource {
                landlord: 3,
                tenant: 2,
                lease_id: 200,
                unit_id: 100,
            },
        };
        state.append_history(history_row).unwrap();
        assert!(state.historical_leases.capacity() <= MAX_HISTORICAL_LEASES);
        assert_eq!(state.waits.capacity(), 0);
        let wait = MoveWaitRecord {
            operation_id: 1,
            household_id: 10,
            city_id: 1,
            assessment_id: 1,
            assessment_version: 1,
            observed_at: GameTime::ZERO,
            reason: MoveWait::EvidenceIncomplete,
        };
        state.record_wait(wait, 0).unwrap();
        assert!(state.waits.capacity() <= MAX_MOVE_WAITS);
        assert!(
            state.owned_capacity_bytes()
                <= size_of::<MoveRuntimeState>()
                    + size_of::<HistoricalLease>() * MAX_HISTORICAL_LEASES
                    + size_of::<MoveWaitRecord>() * MAX_MOVE_WAITS
        );
        println!(
            "housing_move_layout opening_assessment={} opening_candidate={} opening_rows_per_world={} runtime_history_row={} runtime_wait_row={} empty_runtime={} one_history_one_wait_runtime={} option_box_header={} action={}",
            size_of::<OpeningMoveAssessment>(),
            size_of::<OpeningMoveCandidate>(),
            MAX_M0_HOUSING_UNITS
                * (size_of::<OpeningMoveAssessment>() + size_of::<OpeningMoveCandidate>()),
            size_of::<HistoricalLease>(),
            size_of::<MoveWaitRecord>(),
            size_of::<MoveRuntimeState>(),
            state.owned_capacity_bytes(),
            size_of::<Option<Box<MoveRuntimeState>>>(),
            size_of::<crate::transaction::Action>(),
        );
    }

    #[test]
    fn wait_only_backing_is_lazy_and_decode_rejects_row_33() {
        let mut state = MoveRuntimeState::default();
        for household_id in 1..=MAX_MOVE_WAITS as u32 {
            state
                .record_wait(
                    MoveWaitRecord {
                        operation_id: u64::from(household_id),
                        household_id,
                        city_id: 1,
                        assessment_id: u64::from(household_id),
                        assessment_version: 1,
                        observed_at: GameTime::ZERO,
                        reason: MoveWait::EvidenceIncomplete,
                    },
                    0,
                )
                .unwrap();
        }
        assert!(state.historical_leases.is_empty());
        assert_eq!(state.historical_leases.capacity(), 0);
        assert_eq!(state.waits.len(), MAX_MOVE_WAITS);
        assert!(state.waits.capacity() <= MAX_MOVE_WAITS);
        assert!(
            state.owned_capacity_bytes()
                <= size_of::<MoveRuntimeState>() + MAX_MOVE_WAITS * size_of::<MoveWaitRecord>()
        );
        assert_eq!(
            state.record_wait(
                MoveWaitRecord {
                    operation_id: 33,
                    household_id: 33,
                    city_id: 1,
                    assessment_id: 33,
                    assessment_version: 1,
                    observed_at: GameTime::ZERO,
                    reason: MoveWait::EvidenceIncomplete,
                },
                0,
            ),
            Err("housing_move_wait_full")
        );

        let row = serde_json::json!({
            "operation_id":1,
            "household_id":1,
            "city_id":1,
            "assessment_id":1,
            "assessment_version":1,
            "observed_at":0,
            "reason":"evidence_incomplete"
        });
        let thirty_three = serde_json::json!({
            "historical_leases":[],
            "waits":vec![row; MAX_MOVE_WAITS + 1]
        });
        assert!(serde_json::from_value::<MoveRuntimeState>(thirty_three).is_err());
        let thirty_two = serde_json::to_value(&state).unwrap();
        let cold: MoveRuntimeState = serde_json::from_value(thirty_two).unwrap();
        assert_eq!(cold, state);
        assert_eq!(cold.historical_leases.capacity(), 0);
        assert!(cold.waits.capacity() <= MAX_MOVE_WAITS);
    }
}
