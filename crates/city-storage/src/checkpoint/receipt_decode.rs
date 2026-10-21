//! Legacy receipt decoding without internally tagged serde Content graphs.
//! Raw values borrow the retained file; original unknown/duplicate field semantics
//! are preserved. Whitespace and ignored JournalRecord fields are not WAL frames.
use super::*;
use mycity_core::transaction::{Action, Receipt, TxError, MAX_ACTIONS};
use serde::de::{self, MapAccess, Visitor};
use serde_json::value::RawValue;
use std::fmt;

#[derive(Deserialize)]
struct Record {
    actor: u32,
    emergency_eligible: bool,
    operation_id: u64,
    expected_revision: u64,
    payload_sha256: [u8; 32],
    outcome: Result<Receipt, TxError>,
    actions: [Option<SafeAction>; MAX_ACTIONS],
}
struct SafeAction(Action);
impl<'de> Deserialize<'de> for SafeAction {
    fn deserialize<D: serde::Deserializer<'de>>(d: D) -> Result<Self, D::Error> {
        let raw = <&RawValue>::deserialize(d)?;
        action(raw.get()).map(Self).map_err(de::Error::custom)
    }
}
pub(super) fn decode(raw: &str) -> Result<JournalRecord, serde_json::Error> {
    let r: Record = serde_json::from_str(raw)?;
    Ok(JournalRecord {
        actor: r.actor,
        emergency_eligible: r.emergency_eligible,
        operation_id: r.operation_id,
        expected_revision: r.expected_revision,
        payload_sha256: r.payload_sha256,
        outcome: r.outcome,
        actions: r.actions.map(|a| a.map(|a| a.0)),
    })
}
struct Fields<'a, const N: usize> {
    names: &'static [&'static str; N],
    values: [Option<&'a RawValue>; N],
}
impl<'a, const N: usize> Fields<'a, N> {
    fn get<T: Deserialize<'a>>(&self, index: usize) -> Result<T, serde_json::Error> {
        serde_json::from_str(self.raw(index)?)
    }
    fn raw(&self, index: usize) -> Result<&'a str, serde_json::Error> {
        self.values[index]
            .map(|v| v.get())
            .ok_or_else(|| de::Error::missing_field(self.names[index]))
    }
}
fn fields<'a, const N: usize>(
    raw: &'a str,
    names: &'static [&'static str; N],
) -> Result<Fields<'a, N>, serde_json::Error> {
    struct Object<const N: usize>(&'static [&'static str; N]);
    impl<'de, const N: usize> Visitor<'de> for Object<N> {
        type Value = Fields<'de, N>;
        fn expecting(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
            f.write_str("original tagged object")
        }
        fn visit_seq<A: serde::de::SeqAccess<'de>>(
            self,
            mut seq: A,
        ) -> Result<Self::Value, A::Error> {
            let mut result = Fields {
                names: self.0,
                values: [None; N],
            };
            for (i, value) in result.values.iter_mut().enumerate() {
                *value = Some(
                    seq.next_element()?
                        .ok_or_else(|| de::Error::missing_field(self.0[i]))?,
                );
            }
            seq.next_element_seed(crate::metadata::RejectExtra)?;
            Ok(result)
        }
        fn visit_map<A: MapAccess<'de>>(self, mut map: A) -> Result<Self::Value, A::Error> {
            struct Key<const N: usize>(&'static [&'static str; N]);
            impl<'de, const N: usize> serde::de::DeserializeSeed<'de> for Key<N> {
                type Value = usize;
                fn deserialize<D: serde::Deserializer<'de>>(self, d: D) -> Result<usize, D::Error> {
                    d.deserialize_identifier(self)
                }
            }
            impl<const N: usize> Visitor<'_> for Key<N> {
                type Value = usize;
                fn expecting(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
                    f.write_str("known tagged field")
                }
                fn visit_str<E: de::Error>(self, v: &str) -> Result<usize, E> {
                    self.0
                        .iter()
                        .position(|n| *n == v)
                        .ok_or_else(|| E::custom("unknown tagged field"))
                }
            }
            let mut result = Fields {
                names: self.0,
                values: [None; N],
            };
            while let Some(i) = map.next_key_seed(Key(self.0))? {
                if result.values[i].is_some() {
                    return Err(de::Error::duplicate_field(self.0[i]));
                }
                result.values[i] = Some(map.next_value()?);
            }
            Ok(result)
        }
    }
    let mut d = serde_json::Deserializer::from_str(raw);
    let result = serde::Deserializer::deserialize_any(&mut d, Object(names))?;
    d.end()?;
    Ok(result)
}

fn action(raw: &str) -> Result<mycity_core::transaction::Action, serde_json::Error> {
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
    validate(raw)?;
    let kind: Kind = tag(raw)?;
    match kind {
        Kind::Background => {
            let fields = fields(raw, &["kind", "command"])?;
            Ok(Output::Background {
                command: background(fields.raw(1)?)?,
            })
        }
        Kind::Timeline => {
            let fields = fields(raw, &["kind", "command"])?;
            Ok(Output::Timeline {
                command: timeline(fields.raw(1)?)?,
            })
        }
        Kind::Inbox => {
            let fields = fields(raw, &["kind", "command"])?;
            Ok(Output::Inbox {
                command: inbox(fields.raw(1)?)?,
            })
        }
        Kind::Exchange => {
            let fields = fields(raw, &["kind", "command"])?;
            Ok(Output::Exchange {
                command: exchange(fields.raw(1)?)?,
            })
        }
        Kind::Boot => {
            let fields = fields(raw, &["kind", "command"])?;
            Ok(Output::Boot {
                command: boot(fields.raw(1)?)?,
            })
        }
        Kind::Economy => {
            let fields = fields(raw, &["kind", "command"])?;
            Ok(Output::Economy {
                command: economy(fields.raw(1)?)?,
            })
        }
        Kind::RegionSettleDebt => {
            let fields = fields(raw, &["kind", "obligation"])?;
            Ok(Output::RegionSettleDebt {
                obligation: fields.get(1)?,
            })
        }
        Kind::RegionActivate => {
            let fields = fields(raw, &["kind", "city"])?;
            Ok(Output::RegionActivate {
                city: fields.get(1)?,
            })
        }
        Kind::RegionAdvance => {
            let fields = fields(raw, &["kind", "seconds"])?;
            Ok(Output::RegionAdvance {
                seconds: fields.get(1)?,
            })
        }
        Kind::RegionMigrate => {
            let fields = fields(raw, &["kind", "from", "to", "requested", "tolerance"])?;
            Ok(Output::RegionMigrate {
                from: fields.get(1)?,
                to: fields.get(2)?,
                requested: fields.get(3)?,
                tolerance: fields.get(4)?,
            })
        }
        Kind::Transfer => {
            let fields = fields(raw, &["kind", "from", "to", "amount"])?;
            Ok(Output::Transfer {
                from: fields.get(1)?,
                to: fields.get(2)?,
                amount: fields.get(3)?,
            })
        }
        Kind::Protect => {
            let fields = fields(raw, &["kind", "account", "amount"])?;
            Ok(Output::Protect {
                account: fields.get(1)?,
                amount: fields.get(2)?,
            })
        }
        Kind::ReleaseProtection => {
            let fields = fields(raw, &["kind", "account", "amount"])?;
            Ok(Output::ReleaseProtection {
                account: fields.get(1)?,
                amount: fields.get(2)?,
            })
        }
        Kind::PayProtected => {
            let fields = fields(raw, &["kind", "from", "to", "amount"])?;
            Ok(Output::PayProtected {
                from: fields.get(1)?,
                to: fields.get(2)?,
                amount: fields.get(3)?,
            })
        }
        Kind::MoveStock => {
            let fields = fields(raw, &["kind", "from", "to", "quantity"])?;
            Ok(Output::MoveStock {
                from: fields.get(1)?,
                to: fields.get(2)?,
                quantity: fields.get(3)?,
            })
        }
        Kind::ReserveCapacity => {
            let fields = fields(raw, &["kind", "resource", "units"])?;
            Ok(Output::ReserveCapacity {
                resource: fields.get(1)?,
                units: fields.get(2)?,
            })
        }
        Kind::OccupyCapacity => {
            let fields = fields(raw, &["kind", "token"])?;
            Ok(Output::OccupyCapacity {
                token: fields.get(1)?,
            })
        }
        Kind::ReleaseCapacity => {
            let fields = fields(raw, &["kind", "token"])?;
            Ok(Output::ReleaseCapacity {
                token: fields.get(1)?,
            })
        }
    }
}

fn background(raw: &str) -> Result<mycity_core::background::Command, serde_json::Error> {
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
    validate(raw)?;
    let kind: Kind = tag(raw)?;
    match kind {
        Kind::Enable => {
            unit(raw)?;
            Ok(Output::Enable)
        }
        Kind::Restart => {
            unit(raw)?;
            Ok(Output::Restart)
        }
        Kind::Lease => {
            let fields = fields(raw, &["kind", "job"])?;
            Ok(Output::Lease {
                job: fields.get(1)?,
            })
        }
        Kind::RebindLease => {
            let fields = fields(raw, &["kind", "job"])?;
            Ok(Output::RebindLease {
                job: fields.get(1)?,
            })
        }
        Kind::Fail => {
            let fields = fields(raw, &["kind", "job", "generation", "failure"])?;
            Ok(Output::Fail {
                job: fields.get(1)?,
                generation: fields.get(2)?,
                failure: fields.get(3)?,
            })
        }
        Kind::Accept => {
            let fields = fields(raw, &["kind", "job", "generation", "candidate_sha256"])?;
            Ok(Output::Accept {
                job: fields.get(1)?,
                generation: fields.get(2)?,
                candidate_sha256: fields.get(3)?,
            })
        }
    }
}

fn timeline(raw: &str) -> Result<mycity_core::timeline::Command, serde_json::Error> {
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
        SettleClaim,
        SettlePayroll,
        Advance,
    }
    validate(raw)?;
    let kind: Kind = tag(raw)?;
    match kind {
        Kind::Enable => {
            unit(raw)?;
            Ok(Output::Enable)
        }
        Kind::CancelAdvance => {
            unit(raw)?;
            Ok(Output::CancelAdvance)
        }
        Kind::Claims => {
            let fields = fields(
                raw,
                &[
                    "kind", "first_id", "count", "payer", "payee", "amount", "due",
                ],
            )?;
            Ok(Output::Claims {
                first_id: fields.get(1)?,
                count: fields.get(2)?,
                payer: fields.get(3)?,
                payee: fields.get(4)?,
                amount: fields.get(5)?,
                due: fields.get(6)?,
            })
        }
        Kind::Lend => {
            let fields = fields(
                raw,
                &["kind", "id", "lender", "borrower", "principal", "due"],
            )?;
            Ok(Output::Lend {
                id: fields.get(1)?,
                lender: fields.get(2)?,
                borrower: fields.get(3)?,
                principal: fields.get(4)?,
                due: fields.get(5)?,
            })
        }
        Kind::AddPayroll => {
            let fields = fields(raw, &["kind", "spec"])?;
            Ok(Output::AddPayroll {
                spec: fields.get(1)?,
            })
        }
        Kind::StopWork => {
            let fields = fields(raw, &["kind", "id"])?;
            Ok(Output::StopWork { id: fields.get(1)? })
        }
        Kind::ChangeCycle => {
            let fields = fields(raw, &["kind", "id", "cycle"])?;
            Ok(Output::ChangeCycle {
                id: fields.get(1)?,
                cycle: fields.get(2)?,
            })
        }
        Kind::Terminate => {
            let fields = fields(raw, &["kind", "id"])?;
            Ok(Output::Terminate { id: fields.get(1)? })
        }
        Kind::SettleClaim => {
            let fields = fields(raw, &["kind", "id"])?;
            Ok(Output::SettleClaim { id: fields.get(1)? })
        }
        Kind::SettlePayroll => {
            let fields = fields(raw, &["kind", "id"])?;
            Ok(Output::SettlePayroll { id: fields.get(1)? })
        }
        Kind::Advance => {
            let fields = fields(raw, &["kind", "target", "limit"])?;
            Ok(Output::Advance {
                target: fields.get(1)?,
                limit: fields.get(2)?,
            })
        }
    }
}

fn inbox(raw: &str) -> Result<mycity_core::inbox::Command, serde_json::Error> {
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
    validate(raw)?;
    let kind: Kind = tag(raw)?;
    match kind {
        Kind::Enable => {
            unit(raw)?;
            Ok(Output::Enable)
        }
        Kind::External => {
            let fields = fields(raw, &["kind", "fact"])?;
            Ok(Output::External {
                fact: fields.get(1)?,
            })
        }
        Kind::Receive => {
            let fields = fields(raw, &["kind", "quote"])?;
            Ok(Output::Receive {
                quote: fields.get(1)?,
            })
        }
        Kind::Incident => {
            let fields = fields(raw, &["kind", "quote"])?;
            Ok(Output::Incident {
                quote: fields.get(1)?,
            })
        }
        Kind::Settle => {
            let fields = fields(raw, &["kind", "sequence"])?;
            Ok(Output::Settle {
                sequence: fields.get(1)?,
            })
        }
        Kind::Handoff => {
            let fields = fields(raw, &["kind", "sequence", "manager"])?;
            Ok(Output::Handoff {
                sequence: fields.get(1)?,
                manager: fields.get(2)?,
            })
        }
        Kind::Archive => {
            let fields = fields(raw, &["kind", "sequence"])?;
            Ok(Output::Archive {
                sequence: fields.get(1)?,
            })
        }
    }
}

fn exchange(raw: &str) -> Result<mycity_core::exchange::Command, serde_json::Error> {
    use mycity_core::exchange::Command as Output;
    #[derive(Deserialize)]
    #[serde(rename_all = "snake_case")]
    enum Kind {
        Dispatch,
        SetLink,
        Cancel,
    }
    validate(raw)?;
    let kind: Kind = tag(raw)?;
    match kind {
        Kind::Dispatch => {
            let fields = fields(raw, &["kind", "quote"])?;
            Ok(Output::Dispatch {
                quote: fields.get(1)?,
            })
        }
        Kind::SetLink => {
            let fields = fields(raw, &["kind", "edge", "open"])?;
            Ok(Output::SetLink {
                edge: fields.get(1)?,
                open: fields.get(2)?,
            })
        }
        Kind::Cancel => {
            let fields = fields(raw, &["kind", "quote"])?;
            Ok(Output::Cancel {
                quote: fields.get(1)?,
            })
        }
    }
}

fn boot(raw: &str) -> Result<mycity_core::boot::Command, serde_json::Error> {
    use mycity_core::boot::Command as Output;
    #[derive(Deserialize)]
    #[serde(rename_all = "snake_case")]
    enum Kind {
        Start,
        Advance,
        Fault,
        ClearFailure,
    }
    validate(raw)?;
    let kind: Kind = tag(raw)?;
    match kind {
        Kind::Start => {
            let fields = fields(raw, &["kind", "facility", "reverse_request"])?;
            Ok(Output::Start {
                facility: fields.get(1)?,
                reverse_request: fields.get(2)?,
            })
        }
        Kind::Advance => {
            let fields = fields(raw, &["kind", "seconds"])?;
            Ok(Output::Advance {
                seconds: fields.get(1)?,
            })
        }
        Kind::Fault => {
            let fields = fields(raw, &["kind", "facility"])?;
            Ok(Output::Fault {
                facility: fields.get(1)?,
            })
        }
        Kind::ClearFailure => {
            let fields = fields(raw, &["kind", "facility"])?;
            Ok(Output::ClearFailure {
                facility: fields.get(1)?,
            })
        }
    }
}

fn economy(raw: &str) -> Result<mycity_core::economy::Command, serde_json::Error> {
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
    validate(raw)?;
    let kind: Kind = tag(raw)?;
    match kind {
        Kind::Advance => {
            let fields = fields(raw, &["kind", "seconds"])?;
            Ok(Output::Advance {
                seconds: fields.get(1)?,
            })
        }
        Kind::SuspendShift => {
            unit(raw)?;
            Ok(Output::SuspendShift)
        }
        Kind::Prepay => {
            let fields = fields(raw, &["kind", "quantity"])?;
            Ok(Output::Prepay {
                quantity: fields.get(1)?,
            })
        }
        Kind::CancelFuture => {
            unit(raw)?;
            Ok(Output::CancelFuture)
        }
        Kind::SettleDebt => {
            let fields = fields(raw, &["kind", "claim"])?;
            Ok(Output::SettleDebt {
                claim: fields.get(1)?,
            })
        }
        Kind::HandoffDebt => {
            let fields = fields(raw, &["kind", "claim"])?;
            Ok(Output::HandoffDebt {
                claim: fields.get(1)?,
            })
        }
        Kind::BeginExit => {
            unit(raw)?;
            Ok(Output::BeginExit)
        }
        Kind::Close => {
            unit(raw)?;
            Ok(Output::Close)
        }
    }
}

// This streaming validation mirrors Content's scalar decoding, without building a
// Content tree. RawValue/IgnoredAny alone can overlook an overflowing float or a
// malformed surrogate in a subtree that an internally tagged unit later ignores.
fn validate(raw: &str) -> Result<(), serde_json::Error> {
    struct Check;
    impl<'de> serde::de::DeserializeSeed<'de> for Check {
        type Value = ();
        fn deserialize<D: serde::Deserializer<'de>>(self, d: D) -> Result<(), D::Error> {
            d.deserialize_any(self)
        }
    }
    impl<'de> Visitor<'de> for Check {
        type Value = ();
        fn expecting(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
            f.write_str("valid Content scalar or container")
        }
        fn visit_bool<E: de::Error>(self, _: bool) -> Result<(), E> {
            Ok(())
        }
        fn visit_i64<E: de::Error>(self, _: i64) -> Result<(), E> {
            Ok(())
        }
        fn visit_u64<E: de::Error>(self, _: u64) -> Result<(), E> {
            Ok(())
        }
        fn visit_f64<E: de::Error>(self, _: f64) -> Result<(), E> {
            Ok(())
        }
        fn visit_str<E: de::Error>(self, _: &str) -> Result<(), E> {
            Ok(())
        }
        fn visit_unit<E: de::Error>(self) -> Result<(), E> {
            Ok(())
        }
        fn visit_seq<A: serde::de::SeqAccess<'de>>(self, mut seq: A) -> Result<(), A::Error> {
            while seq.next_element_seed(Check)?.is_some() {}
            Ok(())
        }
        fn visit_map<A: MapAccess<'de>>(self, mut map: A) -> Result<(), A::Error> {
            while map.next_key_seed(Check)?.is_some() {
                map.next_value_seed(Check)?;
            }
            Ok(())
        }
    }
    let mut d = serde_json::Deserializer::from_str(raw);
    serde::de::DeserializeSeed::deserialize(Check, &mut d)?;
    d.end()
}
fn tag<'de, T: Deserialize<'de>>(raw: &'de str) -> Result<T, serde_json::Error> {
    struct Tag<T>(std::marker::PhantomData<T>);
    impl<'de, T: Deserialize<'de>> Visitor<'de> for Tag<T> {
        type Value = T;
        fn expecting(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
            f.write_str("internally tagged map or sequence")
        }
        fn visit_seq<A: serde::de::SeqAccess<'de>>(self, mut seq: A) -> Result<T, A::Error> {
            let tag = seq
                .next_element()?
                .ok_or_else(|| de::Error::missing_field("kind"))?;
            while seq.next_element::<&RawValue>()?.is_some() {}
            Ok(tag)
        }
        fn visit_map<A: MapAccess<'de>>(self, mut map: A) -> Result<T, A::Error> {
            struct KeyVisitor;
            impl Visitor<'_> for KeyVisitor {
                type Value = bool;
                fn expecting(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
                    f.write_str("tag or content key")
                }
                fn visit_str<E: de::Error>(self, v: &str) -> Result<bool, E> {
                    Ok(v == "kind")
                }
            }
            struct KeySeed;
            impl<'de> serde::de::DeserializeSeed<'de> for KeySeed {
                type Value = bool;
                fn deserialize<D: serde::Deserializer<'de>>(self, d: D) -> Result<bool, D::Error> {
                    d.deserialize_identifier(KeyVisitor)
                }
            }
            let mut tag = None;
            while let Some(is_kind) = map.next_key_seed(KeySeed)? {
                if is_kind {
                    if tag.is_some() {
                        return Err(de::Error::duplicate_field("kind"));
                    }
                    tag = Some(map.next_value()?);
                } else {
                    let _ = map.next_value::<&RawValue>()?;
                }
            }
            tag.ok_or_else(|| de::Error::missing_field("kind"))
        }
    }
    let mut d = serde_json::Deserializer::from_str(raw);
    let result = serde::Deserializer::deserialize_any(&mut d, Tag(std::marker::PhantomData))?;
    d.end()?;
    Ok(result)
}
fn unit(raw: &str) -> Result<(), serde_json::Error> {
    // Map unit variants ignore arbitrary fields, but duplicate tags were already
    // rejected. Content's sequence end check accepts only the tag with no tail.
    if raw.trim_start().starts_with('[') {
        let _ = fields(raw, &["kind"])?;
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
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
            r#"{"kind":"timeline","command":{"kind":"settle_claim","id":1}}"#,
            r#"{"kind":"timeline","command":{"kind":"settle_payroll","id":1}}"#,
            r#"{"kind":"timeline","command":{"kind":"advance","target":1,"limit":1}}"#,
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
                let source = fields(raw, &["kind", "command"]).unwrap();
                let mut parser = serde_json::Deserializer::from_str(source.raw(1).unwrap());
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
