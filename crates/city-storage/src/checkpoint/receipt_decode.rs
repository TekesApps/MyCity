//! Legacy receipt decoding with fixed lexical backing and borrowed spans.
use super::*;
use crate::metadata::{
    lexical::{validate_content, Array, Fields, Raw, Scratch},
    reader,
};
#[cfg(test)]
use mycity_core::transaction::Action;
use mycity_core::transaction::MAX_ACTIONS;
use serde::de;

pub(crate) fn decode_raw(
    raw: Raw<'_>,
    scratch: &mut Scratch,
) -> Result<JournalRecord, serde_json::Error> {
    let fields = Fields::parse(
        raw,
        &[
            "actor",
            "emergency_eligible",
            "operation_id",
            "expected_revision",
            "payload_sha256",
            "outcome",
            "actions",
        ],
        true,
        scratch,
    )?;
    let mut actions = [None; MAX_ACTIONS];
    let mut array = Array::new(fields.raw(6)?)?;
    for slot in &mut actions {
        let raw = array
            .next(scratch)?
            .ok_or_else(|| crate::metadata::lexical::error("receipt_action_count"))?;
        if raw.bytes != b"null" {
            *slot = Some(action_raw(raw, scratch)?);
        }
    }
    if array.next(scratch)?.is_some() {
        return Err(crate::metadata::lexical::error("receipt_action_count"));
    }
    Ok(JournalRecord {
        actor: fields.get(0, scratch)?,
        emergency_eligible: fields.get(1, scratch)?,
        operation_id: fields.get(2, scratch)?,
        expected_revision: fields.get(3, scratch)?,
        payload_sha256: fields.get(4, scratch)?,
        outcome: fields.get(5, scratch)?,
        actions,
    })
}
#[cfg(test)]
fn decode(raw: &str) -> Result<JournalRecord, serde_json::Error> {
    decode_raw(Raw::root(raw.as_bytes()), &mut Scratch::new()?)
}
#[cfg(test)]
fn action(raw: &str) -> Result<Action, serde_json::Error> {
    action_raw(Raw::root(raw.as_bytes()), &mut Scratch::new()?)
}
fn tag<'de, T: Deserialize<'de>>(
    raw: Raw<'de>,
    scratch: &mut Scratch,
) -> Result<T, serde_json::Error> {
    let mut cursor = crate::metadata::lexical::Cursor::new(raw)?;
    if cursor.peek() == Some(b'[') {
        let mut array = Array::new(raw)?;
        let first = array
            .next(scratch)?
            .ok_or_else(|| de::Error::missing_field("kind"))?;
        reader::decode(first, scratch)
    } else {
        Fields::parse(raw, &["kind"], true, scratch)?.get(0, scratch)
    }
}
fn unit(raw: Raw<'_>, scratch: &mut Scratch) -> Result<(), serde_json::Error> {
    let mut cursor = crate::metadata::lexical::Cursor::new(raw)?;
    if cursor.peek() == Some(b'[') {
        let _ = Fields::parse(raw, &["kind"], false, scratch)?;
    }
    Ok(())
}
fn action_raw(
    raw: Raw<'_>,
    scratch: &mut Scratch,
) -> Result<mycity_core::transaction::Action, serde_json::Error> {
    use mycity_core::transaction::Action as Output;
    #[derive(Deserialize)]
    #[serde(rename_all = "snake_case")]
    enum Kind {
        Background,
        Timeline,
        Inbox,
        Exchange,
        Boot,
        Economy,
        Construction,
        TownSettle,
        TownTransportSettle,
        TownSuspendOperations,
        BeginExit,
        SettleExit,
        CloseExit,
        RegionSettleDebt,
        RegionActivate,
        RegionAdvance,
        RegionMigrate,
        Transfer,
        Protect,
        ReleaseProtection,
        PayProtected,
        MoveStock,
        ReserveCapacity,
        OccupyCapacity,
        ReleaseCapacity,
    }
    validate_content(raw, scratch)?;
    let kind: Kind = tag(raw, scratch)?;
    match kind {
        Kind::Background => {
            let fields = Fields::parse(raw, &["kind", "command"], false, scratch)?;
            Ok(Output::Background {
                command: background(fields.raw(1)?, scratch)?,
            })
        }
        Kind::Timeline => {
            let fields = Fields::parse(raw, &["kind", "command"], false, scratch)?;
            Ok(Output::Timeline {
                command: timeline(fields.raw(1)?, scratch)?,
            })
        }
        Kind::Inbox => {
            let fields = Fields::parse(raw, &["kind", "command"], false, scratch)?;
            Ok(Output::Inbox {
                command: inbox(fields.raw(1)?, scratch)?,
            })
        }
        Kind::Exchange => {
            let fields = Fields::parse(raw, &["kind", "command"], false, scratch)?;
            Ok(Output::Exchange {
                command: exchange(fields.raw(1)?, scratch)?,
            })
        }
        Kind::Boot => {
            let fields = Fields::parse(raw, &["kind", "command"], false, scratch)?;
            Ok(Output::Boot {
                command: boot(fields.raw(1)?, scratch)?,
            })
        }
        Kind::Economy => {
            let fields = Fields::parse(raw, &["kind", "command"], false, scratch)?;
            Ok(Output::Economy {
                command: economy(fields.raw(1)?, scratch)?,
            })
        }
        Kind::Construction => {
            let fields = Fields::parse(raw, &["kind", "command"], false, scratch)?;
            Ok(Output::Construction {
                command: reader::decode(fields.raw(1)?, scratch)?,
            })
        }
        Kind::TownSettle => {
            let fields = Fields::parse(raw, &["kind", "profile_id"], false, scratch)?;
            Ok(Output::TownSettle {
                profile_id: fields.get(1, scratch)?,
            })
        }
        Kind::TownTransportSettle => {
            let fields = Fields::parse(
                raw,
                &[
                    "kind",
                    "profile_id",
                    "service_day",
                    "generation",
                    "proof_sha256",
                ],
                false,
                scratch,
            )?;
            Ok(Output::TownTransportSettle {
                profile_id: fields.get(1, scratch)?,
                service_day: fields.get(2, scratch)?,
                generation: fields.get(3, scratch)?,
                proof_sha256: fields.get(4, scratch)?,
            })
        }
        Kind::TownSuspendOperations => {
            let fields = Fields::parse(
                raw,
                &["kind", "profile_id", "company_account"],
                false,
                scratch,
            )?;
            Ok(Output::TownSuspendOperations {
                profile_id: fields.get(1, scratch)?,
                company_account: fields.get(2, scratch)?,
            })
        }
        Kind::BeginExit => {
            let fields = Fields::parse(raw, &["kind", "company", "policy_id"], false, scratch)?;
            Ok(Output::BeginExit {
                company: fields.get(1, scratch)?,
                policy_id: fields.get(2, scratch)?,
            })
        }
        Kind::SettleExit => {
            let fields = Fields::parse(raw, &["kind", "company"], false, scratch)?;
            Ok(Output::SettleExit {
                company: fields.get(1, scratch)?,
            })
        }
        Kind::CloseExit => {
            let fields = Fields::parse(raw, &["kind", "company"], false, scratch)?;
            Ok(Output::CloseExit {
                company: fields.get(1, scratch)?,
            })
        }
        Kind::RegionSettleDebt => {
            let fields = Fields::parse(raw, &["kind", "obligation"], false, scratch)?;
            Ok(Output::RegionSettleDebt {
                obligation: fields.get(1, scratch)?,
            })
        }
        Kind::RegionActivate => {
            let fields = Fields::parse(raw, &["kind", "city"], false, scratch)?;
            Ok(Output::RegionActivate {
                city: fields.get(1, scratch)?,
            })
        }
        Kind::RegionAdvance => {
            let fields = Fields::parse(raw, &["kind", "seconds"], false, scratch)?;
            Ok(Output::RegionAdvance {
                seconds: fields.get(1, scratch)?,
            })
        }
        Kind::RegionMigrate => {
            let fields = Fields::parse(
                raw,
                &["kind", "from", "to", "requested", "tolerance"],
                false,
                scratch,
            )?;
            Ok(Output::RegionMigrate {
                from: fields.get(1, scratch)?,
                to: fields.get(2, scratch)?,
                requested: fields.get(3, scratch)?,
                tolerance: fields.get(4, scratch)?,
            })
        }
        Kind::Transfer => {
            let fields = Fields::parse(raw, &["kind", "from", "to", "amount"], false, scratch)?;
            Ok(Output::Transfer {
                from: fields.get(1, scratch)?,
                to: fields.get(2, scratch)?,
                amount: fields.get(3, scratch)?,
            })
        }
        Kind::Protect => {
            let fields = Fields::parse(raw, &["kind", "account", "amount"], false, scratch)?;
            Ok(Output::Protect {
                account: fields.get(1, scratch)?,
                amount: fields.get(2, scratch)?,
            })
        }
        Kind::ReleaseProtection => {
            let fields = Fields::parse(raw, &["kind", "account", "amount"], false, scratch)?;
            Ok(Output::ReleaseProtection {
                account: fields.get(1, scratch)?,
                amount: fields.get(2, scratch)?,
            })
        }
        Kind::PayProtected => {
            let fields = Fields::parse(raw, &["kind", "from", "to", "amount"], false, scratch)?;
            Ok(Output::PayProtected {
                from: fields.get(1, scratch)?,
                to: fields.get(2, scratch)?,
                amount: fields.get(3, scratch)?,
            })
        }
        Kind::MoveStock => {
            let fields = Fields::parse(raw, &["kind", "from", "to", "quantity"], false, scratch)?;
            Ok(Output::MoveStock {
                from: fields.get(1, scratch)?,
                to: fields.get(2, scratch)?,
                quantity: fields.get(3, scratch)?,
            })
        }
        Kind::ReserveCapacity => {
            let fields = Fields::parse(raw, &["kind", "resource", "units"], false, scratch)?;
            Ok(Output::ReserveCapacity {
                resource: fields.get(1, scratch)?,
                units: fields.get(2, scratch)?,
            })
        }
        Kind::OccupyCapacity => {
            let fields = Fields::parse(raw, &["kind", "token"], false, scratch)?;
            Ok(Output::OccupyCapacity {
                token: fields.get(1, scratch)?,
            })
        }
        Kind::ReleaseCapacity => {
            let fields = Fields::parse(raw, &["kind", "token"], false, scratch)?;
            Ok(Output::ReleaseCapacity {
                token: fields.get(1, scratch)?,
            })
        }
    }
}

fn background(
    raw: Raw<'_>,
    scratch: &mut Scratch,
) -> Result<mycity_core::background::Command, serde_json::Error> {
    use mycity_core::background::Command as Output;
    #[derive(Deserialize)]
    #[serde(rename_all = "snake_case")]
    enum Kind {
        Enable,
        Restart,
        Lease,
        RebindLease,
        Fail,
        Accept,
    }
    validate_content(raw, scratch)?;
    let kind: Kind = tag(raw, scratch)?;
    match kind {
        Kind::Enable => {
            unit(raw, scratch)?;
            Ok(Output::Enable)
        }
        Kind::Restart => {
            unit(raw, scratch)?;
            Ok(Output::Restart)
        }
        Kind::Lease => {
            let fields = Fields::parse(raw, &["kind", "job"], false, scratch)?;
            Ok(Output::Lease {
                job: fields.get(1, scratch)?,
            })
        }
        Kind::RebindLease => {
            let fields = Fields::parse(raw, &["kind", "job"], false, scratch)?;
            Ok(Output::RebindLease {
                job: fields.get(1, scratch)?,
            })
        }
        Kind::Fail => {
            let fields = Fields::parse(
                raw,
                &["kind", "job", "generation", "failure"],
                false,
                scratch,
            )?;
            Ok(Output::Fail {
                job: fields.get(1, scratch)?,
                generation: fields.get(2, scratch)?,
                failure: fields.get(3, scratch)?,
            })
        }
        Kind::Accept => {
            let fields = Fields::parse(
                raw,
                &["kind", "job", "generation", "candidate_sha256"],
                false,
                scratch,
            )?;
            Ok(Output::Accept {
                job: fields.get(1, scratch)?,
                generation: fields.get(2, scratch)?,
                candidate_sha256: fields.get(3, scratch)?,
            })
        }
    }
}

fn timeline(
    raw: Raw<'_>,
    scratch: &mut Scratch,
) -> Result<mycity_core::timeline::Command, serde_json::Error> {
    use mycity_core::timeline::Command as Output;
    #[derive(Deserialize)]
    #[serde(rename_all = "snake_case")]
    enum Kind {
        Enable,
        CancelAdvance,
        Claims,
        Lend,
        AddPayroll,
        StopWork,
        ChangeCycle,
        Terminate,
        EndForExit,
        SettleClaim,
        SettlePayroll,
        Advance,
    }
    validate_content(raw, scratch)?;
    let kind: Kind = tag(raw, scratch)?;
    match kind {
        Kind::Enable => {
            unit(raw, scratch)?;
            Ok(Output::Enable)
        }
        Kind::CancelAdvance => {
            unit(raw, scratch)?;
            Ok(Output::CancelAdvance)
        }
        Kind::Claims => {
            let fields = Fields::parse(
                raw,
                &[
                    "kind", "first_id", "count", "payer", "payee", "amount", "due",
                ],
                false,
                scratch,
            )?;
            Ok(Output::Claims {
                first_id: fields.get(1, scratch)?,
                count: fields.get(2, scratch)?,
                payer: fields.get(3, scratch)?,
                payee: fields.get(4, scratch)?,
                amount: fields.get(5, scratch)?,
                due: fields.get(6, scratch)?,
            })
        }
        Kind::Lend => {
            let fields = Fields::parse(
                raw,
                &["kind", "id", "lender", "borrower", "principal", "due"],
                false,
                scratch,
            )?;
            Ok(Output::Lend {
                id: fields.get(1, scratch)?,
                lender: fields.get(2, scratch)?,
                borrower: fields.get(3, scratch)?,
                principal: fields.get(4, scratch)?,
                due: fields.get(5, scratch)?,
            })
        }
        Kind::AddPayroll => {
            let fields = Fields::parse(raw, &["kind", "spec"], false, scratch)?;
            Ok(Output::AddPayroll {
                spec: fields.get(1, scratch)?,
            })
        }
        Kind::StopWork => {
            let fields = Fields::parse(raw, &["kind", "id"], false, scratch)?;
            Ok(Output::StopWork {
                id: fields.get(1, scratch)?,
            })
        }
        Kind::ChangeCycle => {
            let fields = Fields::parse(raw, &["kind", "id", "cycle"], false, scratch)?;
            Ok(Output::ChangeCycle {
                id: fields.get(1, scratch)?,
                cycle: fields.get(2, scratch)?,
            })
        }
        Kind::Terminate => {
            let fields = Fields::parse(raw, &["kind", "id"], false, scratch)?;
            Ok(Output::Terminate {
                id: fields.get(1, scratch)?,
            })
        }
        Kind::EndForExit => {
            let fields = Fields::parse(raw, &["kind", "id"], false, scratch)?;
            Ok(Output::EndForExit {
                id: fields.get(1, scratch)?,
            })
        }
        Kind::SettleClaim => {
            let fields = Fields::parse(raw, &["kind", "id"], false, scratch)?;
            Ok(Output::SettleClaim {
                id: fields.get(1, scratch)?,
            })
        }
        Kind::SettlePayroll => {
            let fields = Fields::parse(raw, &["kind", "id"], false, scratch)?;
            Ok(Output::SettlePayroll {
                id: fields.get(1, scratch)?,
            })
        }
        Kind::Advance => {
            let fields = Fields::parse(raw, &["kind", "target", "limit"], false, scratch)?;
            Ok(Output::Advance {
                target: fields.get(1, scratch)?,
                limit: fields.get(2, scratch)?,
            })
        }
    }
}

fn inbox(
    raw: Raw<'_>,
    scratch: &mut Scratch,
) -> Result<mycity_core::inbox::Command, serde_json::Error> {
    use mycity_core::inbox::Command as Output;
    #[derive(Deserialize)]
    #[serde(rename_all = "snake_case")]
    enum Kind {
        Enable,
        External,
        Receive,
        Incident,
        Settle,
        Handoff,
        Archive,
    }
    validate_content(raw, scratch)?;
    let kind: Kind = tag(raw, scratch)?;
    match kind {
        Kind::Enable => {
            unit(raw, scratch)?;
            Ok(Output::Enable)
        }
        Kind::External => {
            let fields = Fields::parse(raw, &["kind", "fact"], false, scratch)?;
            Ok(Output::External {
                fact: fields.get(1, scratch)?,
            })
        }
        Kind::Receive => {
            let fields = Fields::parse(raw, &["kind", "quote"], false, scratch)?;
            Ok(Output::Receive {
                quote: fields.get(1, scratch)?,
            })
        }
        Kind::Incident => {
            let fields = Fields::parse(raw, &["kind", "quote"], false, scratch)?;
            Ok(Output::Incident {
                quote: fields.get(1, scratch)?,
            })
        }
        Kind::Settle => {
            let fields = Fields::parse(raw, &["kind", "sequence"], false, scratch)?;
            Ok(Output::Settle {
                sequence: fields.get(1, scratch)?,
            })
        }
        Kind::Handoff => {
            let fields = Fields::parse(raw, &["kind", "sequence", "manager"], false, scratch)?;
            Ok(Output::Handoff {
                sequence: fields.get(1, scratch)?,
                manager: fields.get(2, scratch)?,
            })
        }
        Kind::Archive => {
            let fields = Fields::parse(raw, &["kind", "sequence"], false, scratch)?;
            Ok(Output::Archive {
                sequence: fields.get(1, scratch)?,
            })
        }
    }
}

fn exchange(
    raw: Raw<'_>,
    scratch: &mut Scratch,
) -> Result<mycity_core::exchange::Command, serde_json::Error> {
    use mycity_core::exchange::Command as Output;
    #[derive(Deserialize)]
    #[serde(rename_all = "snake_case")]
    enum Kind {
        Dispatch,
        SetLink,
        Cancel,
    }
    validate_content(raw, scratch)?;
    let kind: Kind = tag(raw, scratch)?;
    match kind {
        Kind::Dispatch => {
            let fields = Fields::parse(raw, &["kind", "quote"], false, scratch)?;
            Ok(Output::Dispatch {
                quote: fields.get(1, scratch)?,
            })
        }
        Kind::SetLink => {
            let fields = Fields::parse(raw, &["kind", "edge", "open"], false, scratch)?;
            Ok(Output::SetLink {
                edge: fields.get(1, scratch)?,
                open: fields.get(2, scratch)?,
            })
        }
        Kind::Cancel => {
            let fields = Fields::parse(raw, &["kind", "quote"], false, scratch)?;
            Ok(Output::Cancel {
                quote: fields.get(1, scratch)?,
            })
        }
    }
}

fn boot(
    raw: Raw<'_>,
    scratch: &mut Scratch,
) -> Result<mycity_core::boot::Command, serde_json::Error> {
    use mycity_core::boot::Command as Output;
    #[derive(Deserialize)]
    #[serde(rename_all = "snake_case")]
    enum Kind {
        Start,
        Advance,
        Fault,
        ClearFailure,
    }
    validate_content(raw, scratch)?;
    let kind: Kind = tag(raw, scratch)?;
    match kind {
        Kind::Start => {
            let fields = Fields::parse(
                raw,
                &["kind", "facility", "reverse_request"],
                false,
                scratch,
            )?;
            Ok(Output::Start {
                facility: fields.get(1, scratch)?,
                reverse_request: fields.get(2, scratch)?,
            })
        }
        Kind::Advance => {
            let fields = Fields::parse(raw, &["kind", "seconds"], false, scratch)?;
            Ok(Output::Advance {
                seconds: fields.get(1, scratch)?,
            })
        }
        Kind::Fault => {
            let fields = Fields::parse(raw, &["kind", "facility"], false, scratch)?;
            Ok(Output::Fault {
                facility: fields.get(1, scratch)?,
            })
        }
        Kind::ClearFailure => {
            let fields = Fields::parse(raw, &["kind", "facility"], false, scratch)?;
            Ok(Output::ClearFailure {
                facility: fields.get(1, scratch)?,
            })
        }
    }
}

fn economy(
    raw: Raw<'_>,
    scratch: &mut Scratch,
) -> Result<mycity_core::economy::Command, serde_json::Error> {
    use mycity_core::economy::Command as Output;
    #[derive(Deserialize)]
    #[serde(rename_all = "snake_case")]
    enum Kind {
        Advance,
        SuspendShift,
        Prepay,
        CancelFuture,
        SettleDebt,
        HandoffDebt,
        BeginExit,
        Close,
    }
    validate_content(raw, scratch)?;
    let kind: Kind = tag(raw, scratch)?;
    match kind {
        Kind::Advance => {
            let fields = Fields::parse(raw, &["kind", "seconds"], false, scratch)?;
            Ok(Output::Advance {
                seconds: fields.get(1, scratch)?,
            })
        }
        Kind::SuspendShift => {
            unit(raw, scratch)?;
            Ok(Output::SuspendShift)
        }
        Kind::Prepay => {
            let fields = Fields::parse(raw, &["kind", "quantity"], false, scratch)?;
            Ok(Output::Prepay {
                quantity: fields.get(1, scratch)?,
            })
        }
        Kind::CancelFuture => {
            unit(raw, scratch)?;
            Ok(Output::CancelFuture)
        }
        Kind::SettleDebt => {
            let fields = Fields::parse(raw, &["kind", "claim"], false, scratch)?;
            Ok(Output::SettleDebt {
                claim: fields.get(1, scratch)?,
            })
        }
        Kind::HandoffDebt => {
            let fields = Fields::parse(raw, &["kind", "claim"], false, scratch)?;
            Ok(Output::HandoffDebt {
                claim: fields.get(1, scratch)?,
            })
        }
        Kind::BeginExit => {
            unit(raw, scratch)?;
            Ok(Output::BeginExit)
        }
        Kind::Close => {
            unit(raw, scratch)?;
            Ok(Output::Close)
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde::de::{MapAccess, Visitor};
    use serde_json::value::RawValue;
    use std::fmt;
    fn compare_action(raw: &str) {
        let old = serde_json::from_str::<Action>(raw);
        let new = action(raw);
        assert_eq!(
            old.is_ok(),
            new.is_ok(),
            "acceptance differs: {raw}; old={old:?}; new={new:?}"
        );
        if let (Ok(old), Ok(new)) = (old, new) {
            assert_eq!(old, new, "value differs: {raw}");
        }
    }
    fn compare_record(raw: &str) {
        let old = serde_json::from_str::<JournalRecord>(raw);
        let new = decode(raw);
        assert_eq!(
            old.is_ok(),
            new.is_ok(),
            "record acceptance differs: {}; old={old:?}; new={new:?}",
            raw.len()
        );
        if let (Ok(old), Ok(new)) = (old, new) {
            assert_eq!(
                serde_json::to_value(old).unwrap(),
                serde_json::to_value(new).unwrap()
            );
        }
    }
    #[test]
    fn all_variants_match_original_typed_reader_for_map_sequence_and_bad_fields() {
        let cases = [
            r#"{"kind":"background","command":{"kind":"enable"}}"#,
            r#"{"kind":"background","command":{"kind":"restart"}}"#,
            r#"{"kind":"background","command":{"kind":"lease","job":1}}"#,
            r#"{"kind":"background","command":{"kind":"rebind_lease","job":1}}"#,
            r#"{"kind":"background","command":{"kind":"fail","job":1,"generation":1,"failure":"transient"}}"#,
            r#"{"kind":"background","command":{"kind":"accept","job":1,"generation":1,"candidate_sha256":[0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0]}}"#,
            r#"{"kind":"timeline","command":{"kind":"enable"}}"#,
            r#"{"kind":"timeline","command":{"kind":"cancel_advance"}}"#,
            r#"{"kind":"timeline","command":{"kind":"claims","first_id":1,"count":1,"payer":1,"payee":1,"amount":1,"due":1}}"#,
            r#"{"kind":"timeline","command":{"kind":"lend","id":1,"lender":1,"borrower":1,"principal":1,"due":1}}"#,
            r#"{"kind":"timeline","command":{"kind":"add_payroll","spec":{"id":1,"payer":1,"payee":2,"home_city":1,"hourly_cents":1,"cycle":{"kind":"daily"},"ends_at":86400}}}"#,
            r#"{"kind":"timeline","command":{"kind":"stop_work","id":1}}"#,
            r#"{"kind":"timeline","command":{"kind":"change_cycle","id":1,"cycle":{"kind":"daily"}}}"#,
            r#"{"kind":"timeline","command":{"kind":"terminate","id":1}}"#,
            r#"{"kind":"timeline","command":{"kind":"end_for_exit","id":1}}"#,
            r#"{"kind":"timeline","command":{"kind":"settle_claim","id":1}}"#,
            r#"{"kind":"timeline","command":{"kind":"settle_payroll","id":1}}"#,
            r#"{"kind":"timeline","command":{"kind":"advance","target":1,"limit":1}}"#,
            r#"{"kind":"town_transport_settle","profile_id":1,"service_day":1,"generation":1,"proof_sha256":[0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0]}"#,
            r#"{"kind":"town_suspend_operations","profile_id":1,"company_account":3}"#,
            r#"{"kind":"inbox","command":{"kind":"enable"}}"#,
            r#"{"kind":"inbox","command":{"kind":"external","fact":{"source":1,"event":1,"debtor":1,"creditor":2,"amount":1,"due":1,"reason":"emergency_repair"}}}"#,
            r#"{"kind":"inbox","command":{"kind":"receive","quote":1}}"#,
            r#"{"kind":"inbox","command":{"kind":"incident","quote":1}}"#,
            r#"{"kind":"inbox","command":{"kind":"settle","sequence":1}}"#,
            r#"{"kind":"inbox","command":{"kind":"handoff","sequence":1,"manager":1}}"#,
            r#"{"kind":"inbox","command":{"kind":"archive","sequence":1}}"#,
            r#"{"kind":"exchange","command":{"kind":"dispatch","quote":1}}"#,
            r#"{"kind":"exchange","command":{"kind":"set_link","edge":1,"open":true}}"#,
            r#"{"kind":"exchange","command":{"kind":"cancel","quote":1}}"#,
            r#"{"kind":"boot","command":{"kind":"start","facility":"pump","reverse_request":false}}"#,
            r#"{"kind":"boot","command":{"kind":"advance","seconds":1}}"#,
            r#"{"kind":"boot","command":{"kind":"fault","facility":"pump"}}"#,
            r#"{"kind":"boot","command":{"kind":"clear_failure","facility":"pump"}}"#,
            r#"{"kind":"economy","command":{"kind":"advance","seconds":1}}"#,
            r#"{"kind":"economy","command":{"kind":"suspend_shift"}}"#,
            r#"{"kind":"economy","command":{"kind":"prepay","quantity":1}}"#,
            r#"{"kind":"economy","command":{"kind":"cancel_future"}}"#,
            r#"{"kind":"economy","command":{"kind":"settle_debt","claim":1}}"#,
            r#"{"kind":"economy","command":{"kind":"handoff_debt","claim":1}}"#,
            r#"{"kind":"economy","command":{"kind":"begin_exit"}}"#,
            r#"{"kind":"economy","command":{"kind":"close"}}"#,
            r#"{"kind":"region_settle_debt","obligation":1}"#,
            r#"{"kind":"region_activate","city":1}"#,
            r#"{"kind":"region_advance","seconds":1}"#,
            r#"{"kind":"region_migrate","from":1,"to":1,"requested":1,"tolerance":1}"#,
            r#"{"kind":"transfer","from":{"world":1,"branch":1,"domain":1,"index":0,"generation":1},"to":{"world":1,"branch":1,"domain":1,"index":0,"generation":1},"amount":1}"#,
            r#"{"kind":"protect","account":{"world":1,"branch":1,"domain":1,"index":0,"generation":1},"amount":1}"#,
            r#"{"kind":"release_protection","account":{"world":1,"branch":1,"domain":1,"index":0,"generation":1},"amount":1}"#,
            r#"{"kind":"pay_protected","from":{"world":1,"branch":1,"domain":1,"index":0,"generation":1},"to":{"world":1,"branch":1,"domain":1,"index":0,"generation":1},"amount":1}"#,
            r#"{"kind":"move_stock","from":{"world":1,"branch":1,"domain":1,"index":0,"generation":1},"to":{"world":1,"branch":1,"domain":1,"index":0,"generation":1},"quantity":1}"#,
            r#"{"kind":"reserve_capacity","resource":1,"units":1}"#,
            r#"{"kind":"occupy_capacity","token":{"world":1,"branch":1,"number":1}}"#,
            r#"{"kind":"release_capacity","token":{"world":1,"branch":1,"number":1}}"#,
        ];
        for raw in cases {
            let expected = serde_json::from_str::<Action>(raw).unwrap();
            assert_eq!(expected, action(raw).unwrap());
            let value: serde_json::Value = serde_json::from_str(raw).unwrap();
            compare_action(&serde_json::to_string_pretty(&value).unwrap());
            let last = raw.replacen("{\"kind\":", "{\"not_a_kind\":", 1);
            compare_action(&last);
            compare_action(&raw.replacen("\"kind\"", "\"k\\u0069nd\"", 1));
            compare_action(&format!("{{\"kind\":\"transfer\",{}", &raw[1..]));
            compare_action(&format!("{{\"unknown\":[0,1],{}", &raw[1..]));
            // Main action always a struct variant. Reuse original source key
            // order to produce the original derived sequence representation.
            let mut pairs = serde_json::Deserializer::from_str(raw);
            struct Pairs;
            impl<'de> Visitor<'de> for Pairs {
                type Value = Vec<&'de RawValue>;
                fn expecting(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
                    f.write_str("ordered fields")
                }
                fn visit_map<A: MapAccess<'de>>(self, mut map: A) -> Result<Self::Value, A::Error> {
                    let mut values = Vec::new();
                    while map.next_key::<String>()?.is_some() {
                        values.push(map.next_value()?);
                    }
                    Ok(values)
                }
            }
            let values = serde::Deserializer::deserialize_map(&mut pairs, Pairs).unwrap();
            let seq = format!(
                "[{}]",
                values.iter().map(|v| v.get()).collect::<Vec<_>>().join(",")
            );
            compare_action(&seq);
            if let Some(command) = value.get("command") {
                let cmd = command.as_object().unwrap();
                let kind = cmd["kind"].as_str().unwrap();
                let domain = value["kind"].as_str().unwrap();
                let source = Fields::parse(
                    Raw::root(raw.as_bytes()),
                    &["kind", "command"],
                    false,
                    &mut Scratch::new().unwrap(),
                )
                .unwrap();
                let mut parser = serde_json::Deserializer::from_slice(source.raw(1).unwrap().bytes);
                let values = serde::Deserializer::deserialize_map(&mut parser, Pairs).unwrap();
                let sequence = format!(
                    "[{}]",
                    values.iter().map(|v| v.get()).collect::<Vec<_>>().join(",")
                );
                compare_action(&format!("{{\"command\":{sequence},\"kind\":\"{domain}\"}}"));
                for body in [
                    format!("{{\"junk\":[1,2],\"kind\":\"{kind}\"}}"),
                    format!("{{\"junk\":1,\"junk\":2,\"kind\":\"{kind}\"}}"),
                    format!("{{\"junk\":1e400,\"kind\":\"{kind}\"}}"),
                    format!("[\"{kind}\"]"),
                    format!("[\"{kind}\",1]"),
                ] {
                    compare_action(&format!("{{\"command\":{body},\"kind\":\"{domain}\"}}"));
                }
            }
        }
        for number in ["-0", "0.0", "1e0", "18446744073709551616", "1e400"] {
            compare_action(&format!(
                "{{\"kind\":\"region_advance\",\"seconds\":{number}}}"
            ));
        }
    }
    #[test]
    fn large_whitespace_ignored_fields_and_tag_last_keep_legacy_semantics() {
        let r = JournalRecord {
            actor: 0,
            emergency_eligible: false,
            operation_id: 1,
            expected_revision: 0,
            payload_sha256: [0; 32],
            outcome: Err(TxError::StaleRevision),
            actions: [None; MAX_ACTIONS],
        };
        let raw = serde_json::to_string(&r).unwrap();
        compare_record(&raw.replacen("{", &format!("{{{}", " ".repeat(65536)), 1));
        compare_record(&format!(
            "{{\"ignored\":[{}],{}",
            vec!["0"; 65536].join(","),
            &raw[1..]
        ));
        compare_record(&format!(
            "{{\"i\\u0067nored\":\"{}\",{}",
            "\\u0061".repeat(65536),
            &raw[1..]
        ));
        compare_record(&format!("{{\"operation_id\":2,{}", &raw[1..]));
        compare_record(&format!("{{\"operation_\\u0069d\":2,{}", &raw[1..]));
        for body in [
            format!(
                "{{\"junk\":[{}],\"kind\":\"enable\"}}",
                vec!["0"; 65536].join(",")
            ),
            "{\"junk\":1e400,\"kind\":\"enable\"}".into(),
            "{\"junk\":\"\\ud800\",\"kind\":\"enable\"}".into(),
        ] {
            compare_action(&format!("{{\"command\":{body},\"kind\":\"background\"}}"));
        }
    }
}

#[cfg(test)]
mod context_tests {
    use super::*;
    #[derive(Deserialize)]
    struct OriginalCheckpointPrefix {
        receipts: Vec<JournalRecord>,
    }
    #[test]
    fn original_checkpoint_depth_context_and_ignored_depth_are_preserved() {
        let mut scratch = Scratch::new().unwrap();
        let record = JournalRecord {
            actor: 0,
            emergency_eligible: false,
            operation_id: 1,
            expected_revision: 0,
            payload_sha256: [0; 32],
            outcome: Err(TxError::StaleRevision),
            actions: [None; MAX_ACTIONS],
        };
        let mut value = serde_json::to_value(record).unwrap();
        for depth in 116..=132 {
            let nested = format!("{}0{}", "[".repeat(depth), "]".repeat(depth));
            // Construct the actual wire without a DOM parser's own depth gate.
            value["actions"][0] = serde_json::Value::Null;
            let raw = serde_json::to_string(&value).unwrap();
            let replacement=format!("\"actions\":[{{\"command\":{{\"junk\":{nested},\"kind\":\"enable\"}},\"kind\":\"background\"}},");
            let raw = raw.replacen("\"actions\":[null,", &replacement, 1);
            let cp = format!("{{\"receipts\":[{raw}]}}");
            let old = serde_json::from_str::<OriginalCheckpointPrefix>(&cp);
            let new = decode_raw(
                Raw {
                    bytes: raw.as_bytes(),
                    depth: 2,
                },
                &mut scratch,
            );
            assert_eq!(
                old.is_ok(),
                new.is_ok(),
                "CP depth {depth}; old={:?}; new={new:?}",
                old.as_ref().err()
            );
            if let Ok(old) = old {
                assert_eq!(
                    serde_json::to_value(old.receipts[0]).unwrap(),
                    serde_json::to_value(new.unwrap()).unwrap()
                );
            }
        }
        // Ordinary struct unknown content has the old unbounded Ignore depth,
        // rather than Content's 128-level gate.
        let raw = serde_json::to_string(&value).unwrap();
        let nested = format!("{}0{}", "[".repeat(4096), "]".repeat(4096));
        let raw = format!("{{\"unknown\":{nested},{}", &raw[1..]);
        let cp = format!("{{\"receipts\":[{raw}]}}");
        let old = serde_json::from_str::<OriginalCheckpointPrefix>(&cp).unwrap();
        let new = decode_raw(
            Raw {
                bytes: raw.as_bytes(),
                depth: 2,
            },
            &mut scratch,
        )
        .unwrap();
        assert_eq!(
            serde_json::to_value(old.receipts[0]).unwrap(),
            serde_json::to_value(new).unwrap()
        );
    }
}
