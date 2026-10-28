//! Original frame order, canonical borrowed record and two fixed32KiB buffers.
use super::*;
use mycity_core::{
    canonical::{FixedWriter, Workspace},
    transaction::{Action, MAX_ACTIONS},
};
use serde::de::{self, SeqAccess, Visitor};
use serde_json::value::RawValue;
use std::fmt;
pub(super) struct Buffers {
    pub frame: [u8; FRAME_LIMIT],
    pub record: [u8; FRAME_LIMIT],
}
impl Buffers {
    pub fn new() -> Self {
        Self {
            frame: [0; FRAME_LIMIT],
            record: [0; FRAME_LIMIT],
        }
    }
}
#[derive(Serialize)]
struct Borrowed<'a> {
    sequence: u64,
    previous: &'a str,
    envelope: &'a Envelope,
    record: &'a RawValue,
}
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
pub(super) struct Entry<'a> {
    pub sequence: u64,
    #[serde(deserialize_with = "hash_text")]
    pub previous: String,
    #[serde(deserialize_with = "bounded_envelope")]
    pub envelope: Envelope,
    #[serde(borrow)]
    pub record: &'a RawValue,
}
fn hash_text<'de, D: serde::Deserializer<'de>>(d: D) -> Result<String, D::Error> {
    struct Hash;
    impl Visitor<'_> for Hash {
        type Value = String;
        fn expecting(&self, f: &mut fmt::Formatter) -> fmt::Result {
            f.write_str("at most64 decoded hash bytes")
        }
        fn visit_str<E: de::Error>(self, s: &str) -> Result<String, E> {
            if s.len() > 64 {
                return Err(E::custom("wal_hash_text_bound"));
            }
            Ok(s.to_owned())
        }
    }
    d.deserialize_str(Hash)
}
fn actions<'de, D: serde::Deserializer<'de>>(d: D) -> Result<Vec<Action>, D::Error> {
    struct Actions;
    impl<'de> Visitor<'de> for Actions {
        type Value = Vec<Action>;
        fn expecting(&self, f: &mut fmt::Formatter) -> fmt::Result {
            f.write_str("at most16 persisted actions")
        }
        fn visit_seq<A: SeqAccess<'de>>(self, mut a: A) -> Result<Self::Value, A::Error> {
            let mut rows = Vec::new();
            rows.try_reserve_exact(MAX_ACTIONS)
                .map_err(de::Error::custom)?;
            if rows.capacity() != MAX_ACTIONS {
                return Err(de::Error::custom("wal_actions_backing"));
            }
            for _ in 0..MAX_ACTIONS {
                match a.next_element()? {
                    Some(v) => rows.push(v),
                    None => return Ok(rows),
                }
            }
            struct Extra;
            impl<'de> de::DeserializeSeed<'de> for Extra {
                type Value = ();
                fn deserialize<D: serde::Deserializer<'de>>(self, _: D) -> Result<(), D::Error> {
                    Err(de::Error::custom("wal_extra_action"))
                }
            }
            a.next_element_seed(Extra)?;
            Ok(rows)
        }
    }
    d.deserialize_seq(Actions)
}
fn bounded_envelope<'de, D: serde::Deserializer<'de>>(d: D) -> Result<Envelope, D::Error> {
    #[derive(Deserialize)]
    #[serde(deny_unknown_fields)]
    struct Fields {
        world: u64,
        branch: u64,
        operation_id: u64,
        expected_revision: u64,
        #[serde(deserialize_with = "actions")]
        actions: Vec<Action>,
    }
    let f = Fields::deserialize(d)?;
    Ok(Envelope {
        world: f.world,
        branch: f.branch,
        operation_id: f.operation_id,
        expected_revision: f.expected_revision,
        actions: f.actions,
    })
}
pub(super) fn decode(bytes: &[u8]) -> Result<Entry<'_>, Error> {
    // Before RawValue's skip stack or serde token scratch can grow.
    let mut reader = mycity_core::fixed_restore::JsonReader::with_string_limit(bytes, 384);
    io::copy(&mut reader, &mut io::sink())?;
    Ok(serde_json::from_slice(bytes)?)
}
pub(super) fn encode<'a>(
    canonical: &mut Workspace,
    buffers: &'a mut Buffers,
    sequence: u64,
    previous: &str,
    envelope: &Envelope,
    record: &JournalRecord,
) -> Result<&'a [u8], Error> {
    let mut typed = FixedWriter::new(&mut buffers.frame);
    serde_json::to_writer(&mut typed, record)?;
    let mut sorted = FixedWriter::new(&mut buffers.record);
    canonical
        .write(typed.as_slice(), &mut sorted)
        .map_err(Error::Model)?;
    let raw: &RawValue = serde_json::from_slice(sorted.as_slice())?;
    let mut frame = FixedWriter::new(&mut buffers.frame);
    serde_json::to_writer(
        &mut frame,
        &Borrowed {
            sequence,
            previous,
            envelope,
            record: raw,
        },
    )?;
    let len = frame.len();
    Ok(&buffers.frame[..len])
}
pub(super) fn checksum(length: &[u8; 4], payload: &[u8]) -> [u8; 32] {
    use sha2::{Digest, Sha256};
    let mut hash = Sha256::new();
    hash.update(length);
    hash.update(payload);
    hash.finalize().into()
}
/// Compare complete canonical record content, including fields ignored by typed
/// serde visitors. Never compare only typed equality or raw JSON spelling.
pub(super) fn matches(
    canonical: &mut Workspace,
    typed: &JournalRecord,
    scratch: &mut [u8],
    expected: &[u8],
) -> Result<bool, Error> {
    struct Equal<'a> {
        expected: &'a [u8],
        at: usize,
        equal: bool,
    }
    impl Write for Equal<'_> {
        fn write(&mut self, data: &[u8]) -> io::Result<usize> {
            let end = self
                .at
                .checked_add(data.len())
                .ok_or_else(|| io::Error::other("wal_compare_length"))?;
            self.equal &= self.expected.get(self.at..end) == Some(data);
            self.at = end;
            Ok(data.len())
        }
        fn flush(&mut self) -> io::Result<()> {
            Ok(())
        }
    }
    let mut output = FixedWriter::new(scratch);
    serde_json::to_writer(&mut output, typed)?;
    let mut equal = Equal {
        expected,
        at: 0,
        equal: true,
    };
    canonical
        .write(output.as_slice(), &mut equal)
        .map_err(Error::Model)?;
    Ok(equal.equal && equal.at == expected.len())
}

#[cfg(test)]
mod tests {
    use super::*;
    use mycity_core::{
        assembly::assemble,
        transaction::{Caller, Engine, JournalRecord, TxError, MAX_ACTIONS},
        units::Money,
    };
    #[derive(Serialize)]
    struct Old {
        sequence: u64,
        previous: String,
        envelope: Envelope,
        record: serde_json::Value,
    }
    #[test]
    fn every_produced_sample_matches_old_frame_bytes_and_split_checksum() {
        let mut world = Engine::new(
            assemble(include_bytes!("../../../fixtures/m0-e0-opening.json")).unwrap(),
            vec![],
        )
        .unwrap();
        let mut canonical = Workspace::new().unwrap();
        let mut buffers = Buffers::new();
        for id in 1..=32 {
            let request = Envelope {
                world: 1,
                branch: 1,
                operation_id: id,
                expected_revision: if id % 3 == 0 {
                    999
                } else {
                    world.world().view().revision
                },
                actions: vec![Action::Transfer {
                    from: world.world().account_ref(2).unwrap(),
                    to: world.world().account_ref(3).unwrap(),
                    amount: Money::new(id as i64).unwrap(),
                }],
            };
            let _ = world.execute(Caller::Account(2), request.clone());
            let hot = world.journal().last().unwrap();
            let mut actions = [None; MAX_ACTIONS];
            for (index, action) in request.actions.iter().enumerate() {
                actions[index] = Some(*action);
            }
            let record = JournalRecord {
                actor: hot.actor,
                emergency_eligible: hot.emergency_eligible,
                operation_id: hot.operation_id,
                expected_revision: hot.expected_revision,
                payload_sha256: hot.payload_sha256,
                outcome: hot.outcome,
                actions:mycity_core::transaction::JournalActions::plain(actions),
                };
            if id % 3 == 0 {
                assert_eq!(record.outcome, Err(TxError::StaleRevision));
            }
            let bytes = encode(
                &mut canonical,
                &mut buffers,
                id,
                &"a".repeat(64),
                &request,
                &record,
            )
            .unwrap();
            let old = json(&Old {
                sequence: id,
                previous: "a".repeat(64),
                envelope: request,
                record: serde_json::to_value(record).unwrap(),
            })
            .unwrap();
            assert_eq!(bytes, old);
            let length = (bytes.len() as u32).to_le_bytes();
            let mut joined = length.to_vec();
            joined.extend_from_slice(bytes);
            assert_eq!(checksum(&length, bytes), digest(&joined));
        }
    }
    #[test]
    fn persisted_envelope_overflow_rejects_before_extra_action_decode() {
        let text = format!(
            r#"{{"sequence":1,"previous":"a","envelope":{{"world":1,"branch":1,"operation_id":1,"expected_revision":0,"actions":[{},{{"unknown":[[[0]]]}}]}},"record":{{}}}}"#,
            vec![r#"{"kind":"region_activate","city":1}"#; MAX_ACTIONS].join(",")
        );
        assert!(matches!(
            decode(text.as_bytes()),
            Err(Error::Invalid("invalid_json"))
        ));
        let actions_text = format!(
            "[{},{{\"unknown\":0}}]",
            vec![r#"{"kind":"region_activate","city":1}"#; MAX_ACTIONS].join(",")
        );
        let mut de = serde_json::Deserializer::from_str(&actions_text);
        assert!(actions(&mut de)
            .unwrap_err()
            .to_string()
            .contains("wal_extra_action"));
    }
}
