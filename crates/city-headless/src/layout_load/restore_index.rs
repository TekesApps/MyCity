//! Retain only cold-restore authority fields, not an unbounded metadata DOM.
//! Unknown display/report fields remain stream-ignored for old v1/v2 inputs.
use super::*;
use serde::de::{self, SeqAccess, Visitor};
use std::fmt;

pub(super) const RAW_BYTES: usize = 256 * 1024;
pub(super) fn read_raw(mut reader: impl Read) -> Result<Vec<u8>, String> {
    // One initialized allocation, including for a stream without a trusted
    // length. Never grow/realloc the input buffer to discover EOF.
    let mut raw = vec![0; RAW_BYTES];
    let mut used = 0;
    while used < RAW_BYTES {
        let n = reader.read(&mut raw[used..]).map_err(err)?;
        if n == 0 {
            raw.truncate(used);
            return Ok(raw);
        }
        used += n;
    }
    let mut probe = [0];
    if reader.read(&mut probe).map_err(err)? != 0 {
        return Err("layout_index_input_too_large".into());
    }
    Ok(raw)
}

pub(super) fn bounded_text<'de, D: serde::Deserializer<'de>>(d: D) -> Result<String, D::Error> {
    struct Text;
    impl Visitor<'_> for Text {
        type Value = String;
        fn expecting(&self, f: &mut fmt::Formatter) -> fmt::Result {
            f.write_str("layout identifier at most 128 UTF-8 bytes")
        }
        fn visit_str<E: de::Error>(self, s: &str) -> Result<String, E> {
            if s.len() > 128 {
                return Err(E::custom("layout_index_identifier_length"));
            }
            Ok(s.to_owned())
        }
    }
    d.deserialize_str(Text)
}
fn optional_text<'de, D: serde::Deserializer<'de>>(d: D) -> Result<Option<String>, D::Error> {
    #[derive(Deserialize)]
    struct Text(#[serde(deserialize_with = "bounded_text")] String);
    Ok(Option::<Text>::deserialize(d)?.map(|text| text.0))
}

#[derive(Deserialize, Serialize)]
pub(super) struct Index {
    #[serde(deserialize_with = "bounded_text")]
    pub schema: String,
    #[serde(default)]
    pub generation: u64,
    pub manifest: Manifest,
    #[serde(default, deserialize_with = "entries")]
    pub files: Vec<FileEntry>,
}
#[derive(Deserialize, Serialize)]
pub(super) struct Manifest {
    #[serde(deserialize_with = "bounded_text")]
    pub schema: String,
    #[serde(deserialize_with = "bounded_text")]
    pub acceptance_sha256: String,
    #[serde(default, deserialize_with = "optional_text")]
    pub source_sha256: Option<String>,
    pub normal_joint_budget_bytes: usize,
    pub external_owned_charge_bytes: usize,
    pub metadata_and_io_charge_bytes: usize,
    pub cow_reserve_bytes: usize,
    #[serde(default)]
    pub mutation_iteration: Option<u64>,
    #[serde(default)]
    pub padding_payload_bytes: Option<[usize; 7]>,
    #[serde(default)]
    pub native: Option<NativeManifest>,
    #[serde(default)]
    pub r07: Option<super::r07::State>,
    #[serde(default, deserialize_with = "optional_text")]
    pub r07_generator: Option<String>,
}
#[derive(Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub(super) struct NativeManifest {
    #[serde(deserialize_with = "bounded_text")]
    schema: String,
    #[serde(deserialize_with = "bounded_text")]
    scope: String,
    account_arena: ArenaLayout,
    inventory_arena: ArenaLayout,
    // Names are decoded with bounded visitors, never arbitrary String derive.
    fields: [FieldOffset; 13],
    vectors: [VectorLayout; 6],
    owned_capacity_bytes: usize,
}
#[derive(Deserialize, Serialize)]
struct FieldOffset(#[serde(deserialize_with = "bounded_text")] String, usize);
#[derive(Deserialize, Serialize)]
struct VectorLayout(
    #[serde(deserialize_with = "bounded_text")] String,
    usize,
    usize,
    usize,
);
#[derive(Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
struct ArenaLayout {
    free_index_capacity: usize,
    header_bytes: usize,
    live_records: usize,
    owned_capacity_bytes: usize,
    record_bytes: usize,
    slot_bytes: usize,
    slots_capacity: usize,
}
fn entries<'de, D: serde::Deserializer<'de>>(d: D) -> Result<Vec<FileEntry>, D::Error> {
    struct Entries;
    impl<'de> Visitor<'de> for Entries {
        type Value = Vec<FileEntry>;
        fn expecting(&self, f: &mut fmt::Formatter) -> fmt::Result {
            f.write_str("at most 29 frozen layout file entries")
        }
        fn visit_seq<A: SeqAccess<'de>>(self, mut seq: A) -> Result<Self::Value, A::Error> {
            let mut out = Vec::with_capacity(29);
            while out.len() < 29 {
                match seq.next_element()? {
                    Some(entry) => out.push(entry),
                    None => return Ok(out),
                }
            }
            struct Reject;
            impl<'de> serde::de::DeserializeSeed<'de> for Reject {
                type Value = ();
                fn deserialize<E: serde::Deserializer<'de>>(self, _: E) -> Result<(), E::Error> {
                    Err(de::Error::custom("layout_index_excess_files"))
                }
            }
            seq.next_element_seed(Reject)?;
            Ok(out)
        }
    }
    d.deserialize_seq(Entries)
}

pub(super) fn decode(raw: &[u8]) -> Result<Index, String> {
    serde_json::from_slice(raw).map_err(err)
}

#[cfg(test)]
mod tests {
    use super::*;
    fn input(files: Value) -> Value {
        json!({"schema":"mycity-layout-snapshot-v2","manifest":{
            "schema":"mycity-l1-layout-v1","acceptance_sha256":"a".repeat(64),
            "normal_joint_budget_bytes":NORMAL_BYTES,"external_owned_charge_bytes":0,
            "metadata_and_io_charge_bytes":4*MIB,"cow_reserve_bytes":WORK_PAGES*MIB},
            "files":files})
    }
    #[test]
    fn ignored_metadata_is_not_retained_and_entries_are_bounded() {
        let mut value = input(json!([]));
        value["display_only"] = json!({"large":vec![0;10000]});
        let parsed = decode(&serde_json::to_vec(&value).unwrap()).unwrap();
        assert!(serde_json::to_value(parsed)
            .unwrap()
            .get("display_only")
            .is_none());
        let file = json!({"name":"native.json","bytes":1,"sha256":"a".repeat(64)});
        assert!(
            decode(&serde_json::to_vec(&input(json!(vec![file.clone(); 29]))).unwrap()).is_ok()
        );
        assert!(decode(&serde_json::to_vec(&input(json!(vec![file; 30]))).unwrap()).is_err());
        value["schema"] = json!("x".repeat(129));
        assert!(decode(&serde_json::to_vec(&value).unwrap()).is_err());
    }
    #[test]
    fn raw_input_retains_one_fixed_capacity_at_eof_and_rejects_extra_byte() {
        for len in [0, 1, RAW_BYTES - 1, RAW_BYTES] {
            let input = vec![b' '; len];
            let raw = read_raw(&input[..]).unwrap();
            assert_eq!(raw.len(), len);
            assert_eq!(raw.capacity(), RAW_BYTES);
        }
        assert!(read_raw(&vec![0; RAW_BYTES + 1][..]).is_err());
    }
}
