//! Retained file and decoded-field backing. No broker authorization is implied.
pub(crate) mod lexical;
pub(crate) mod reader;
use super::*;
use serde::de::{self, DeserializeSeed, SeqAccess, Visitor};
use std::{fmt, mem::size_of};

pub(crate) struct Bytes {
    data: Vec<u8>,
    used: usize,
}
impl Bytes {
    fn new(capacity: usize) -> Result<Self, Error> {
        let mut data = Vec::new();
        data.try_reserve_exact(capacity)
            .map_err(|_| Error::Invalid("metadata_allocation"))?;
        if data.capacity() != capacity {
            return Err(Error::Invalid("metadata_capacity"));
        }
        data.resize(capacity, 0xa5);
        std::hint::black_box(data.iter().fold(0u64, |n, b| n + u64::from(*b)));
        Ok(Self { data, used: 0 })
    }
    pub fn as_slice(&self) -> &[u8] {
        &self.data[..self.used]
    }
    pub fn read(&mut self, path: &Path, limit: usize) -> Result<(), Error> {
        self.used = 0;
        if limit > self.data.len() {
            return Err(Error::Invalid("metadata_read_capacity"));
        }
        let meta = fs::symlink_metadata(path)?;
        if !meta.is_file() || meta.file_type().is_symlink() || meta.len() > limit as u64 {
            return Err(Error::Invalid("dependency_type_or_length"));
        }
        let result = (|| {
            let mut file = File::open(path)?;
            while self.used < limit {
                let n = file.read(&mut self.data[self.used..limit])?;
                if n == 0 {
                    break;
                }
                self.used += n;
            }
            if file.read(&mut [0; 1])? != 0 || self.used as u64 != meta.len() {
                return Err(Error::Invalid("dependency_changed_or_over_limit"));
            }
            Ok(())
        })();
        if result.is_err() {
            self.used = 0;
        }
        result
    }
    pub fn encode(&mut self, value: &impl Serialize, limit: usize) -> Result<(), Error> {
        self.write_with(limit, |out| {
            serde_json::to_writer(out, value).map_err(Error::from)
        })
    }
    pub fn write_with(
        &mut self,
        limit: usize,
        encode: impl FnOnce(&mut mycity_core::canonical::FixedWriter<'_>) -> Result<(), Error>,
    ) -> Result<(), Error> {
        self.used = 0;
        if limit > self.data.len() {
            return Err(Error::Invalid("metadata_write_capacity"));
        }
        let mut out = mycity_core::canonical::FixedWriter::new(&mut self.data[..limit]);
        encode(&mut out)?;
        self.used = out.len();
        Ok(())
    }
    fn identity(&self) -> (usize, usize) {
        (self.data.as_ptr() as usize, self.data.capacity())
    }
}
pub(crate) struct RejectExtra;
impl<'de> DeserializeSeed<'de> for RejectExtra {
    type Value = ();
    fn deserialize<D: serde::Deserializer<'de>>(self, _: D) -> Result<(), D::Error> {
        Err(de::Error::custom("metadata_extra_element"))
    }
}
pub(crate) struct ByteSeed<'a>(pub &'a mut Bytes, pub usize);
impl<'de> DeserializeSeed<'de> for ByteSeed<'_> {
    type Value = ();
    fn deserialize<D: serde::Deserializer<'de>>(self, d: D) -> Result<(), D::Error> {
        d.deserialize_seq(self)
    }
}
impl<'de> Visitor<'de> for ByteSeed<'_> {
    type Value = ();
    fn expecting(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str("bounded strict u8 array")
    }
    fn visit_seq<A: SeqAccess<'de>>(self, mut seq: A) -> Result<(), A::Error> {
        self.0.used = 0;
        if self.1 > self.0.data.len() {
            return Err(de::Error::custom("metadata_byte_capacity"));
        }
        while self.0.used < self.1 {
            match seq.next_element::<u8>()? {
                Some(byte) => {
                    self.0.data[self.0.used] = byte;
                    self.0.used += 1;
                }
                None => return Ok(()),
            }
        }
        seq.next_element_seed(RejectExtra)?;
        Ok(())
    }
}
pub(crate) struct RowsSeed<'a, T>(pub &'a mut Vec<T>);
impl<'de, T: Deserialize<'de>> DeserializeSeed<'de> for RowsSeed<'_, T> {
    type Value = ();
    fn deserialize<D: serde::Deserializer<'de>>(self, d: D) -> Result<(), D::Error> {
        d.deserialize_seq(self)
    }
}
impl<'de, T: Deserialize<'de>> Visitor<'de> for RowsSeed<'_, T> {
    type Value = ();
    fn expecting(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str("fixed metadata records")
    }
    fn visit_seq<A: SeqAccess<'de>>(self, mut seq: A) -> Result<(), A::Error> {
        self.0.clear();
        while self.0.len() < self.0.capacity() {
            match seq.next_element()? {
                Some(row) => self.0.push(row),
                None => return Ok(()),
            }
        }
        seq.next_element_seed(RejectExtra)?;
        Ok(())
    }
}
pub(crate) fn text<'de, D: serde::Deserializer<'de>, const N: usize>(
    d: D,
) -> Result<String, D::Error> {
    struct Text<const N: usize>;
    impl<const N: usize> Visitor<'_> for Text<N> {
        type Value = String;
        fn expecting(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
            write!(f, "at most {N} decoded bytes")
        }
        fn visit_str<E: de::Error>(self, s: &str) -> Result<String, E> {
            if s.len() > N {
                return Err(E::custom("metadata_text_limit"));
            }
            Ok(s.to_owned())
        }
    }
    d.deserialize_str(Text::<N>)
}
pub(crate) fn guard(bytes: &[u8], token: usize) -> Result<(), Error> {
    let mut reader = mycity_core::fixed_restore::JsonReader::with_string_limit(bytes, token);
    let mut page = [0; 4096];
    while reader.read(&mut page)? != 0 {}
    Ok(())
}
fn rows<T: Clone>(value: T, capacity: usize) -> Result<Vec<T>, Error> {
    let mut out = Vec::new();
    out.try_reserve_exact(capacity)
        .map_err(|_| Error::Invalid("metadata_allocation"))?;
    if out.capacity() != capacity {
        return Err(Error::Invalid("metadata_capacity"));
    }
    out.resize(capacity, value);
    std::hint::black_box(out.as_slice());
    out.clear();
    Ok(out)
}
pub struct MetadataWorkspace {
    pub(crate) file: Bytes,
    pub(crate) engine: Bytes,
    pub(crate) small: Bytes,
    pub(crate) spatial_input: Bytes,
    pub(crate) png: Bytes,
    pub(crate) opening: Bytes,
    pub(crate) receipts: Vec<JournalRecord>,
    pub(crate) operations: Vec<u64>,
    pub(crate) resources: Vec<Resource>,
    pub(crate) parser: lexical::Scratch,
}
#[derive(Debug, PartialEq, Eq, Serialize)]
pub struct MetadataIdentity {
    pub backings: [usize; 10],
    pub byte_capacities: [usize; 6],
    pub receipt_capacity_bytes: usize,
    pub operations_capacity_bytes: usize,
    pub resource_capacity_bytes: usize,
    pub lexical_capacity_bytes: usize,
    pub owned_bytes: usize,
}
impl MetadataWorkspace {
    /// Declared retained heap payload; no allocation or profile-derived credit.
    pub(crate) fn planned_heap_bytes() -> usize {
        lexical::STACK_BYTES
            + checkpoint::CHECKPOINT_LIMIT as usize
            + mycity_core::transaction::ENGINE_BYTE_BUDGET
            + 32 * 1024
            + mycity_map::assets::NORMAL_INPUT_BYTES
            + mycity_map::assets::NORMAL_PNG_BYTES
            + INPUT_LIMIT as usize
            + MAX_RECEIPTS * (size_of::<JournalRecord>() + size_of::<u64>())
            + mycity_core::transaction::MAX_RESOURCES * size_of::<Resource>()
    }
    pub fn new() -> Result<Self, Error> {
        let empty = JournalRecord {
            actor: 0,
            emergency_eligible: false,
            operation_id: 0,
            expected_revision: 0,
            payload_sha256: [0; 32],
            outcome: Err(TxError::InvalidOperation),
            actions: [None; mycity_core::transaction::MAX_ACTIONS],
        };
        Ok(Self {
            file: Bytes::new(checkpoint::CHECKPOINT_LIMIT as usize)?,
            engine: Bytes::new(mycity_core::transaction::ENGINE_BYTE_BUDGET)?,
            small: Bytes::new(32 * 1024)?,
            spatial_input: Bytes::new(mycity_map::assets::NORMAL_INPUT_BYTES)?,
            png: Bytes::new(mycity_map::assets::NORMAL_PNG_BYTES)?,
            opening: Bytes::new(INPUT_LIMIT as usize)?,
            receipts: rows(empty, MAX_RECEIPTS)?,
            operations: rows(0, MAX_RECEIPTS)?,
            parser: lexical::Scratch::new()?,
            resources: rows(
                Resource::new(1, 1).map_err(|_| Error::Invalid("metadata_resource_seed"))?,
                mycity_core::transaction::MAX_RESOURCES,
            )?,
        })
    }
    pub fn identity(&self) -> MetadataIdentity {
        let slots = [
            self.file.identity(),
            self.engine.identity(),
            self.small.identity(),
            self.spatial_input.identity(),
            self.png.identity(),
            self.opening.identity(),
        ];
        let byte_capacities = slots.map(|s| s.1);
        let receipt_capacity_bytes = self.receipts.capacity() * size_of::<JournalRecord>();
        let operations_capacity_bytes = self.operations.capacity() * size_of::<u64>();
        let resource_capacity_bytes = self.resources.capacity() * size_of::<Resource>();
        let (parser_address, lexical_capacity_bytes) = self.parser.identity();
        MetadataIdentity {
            backings: [
                slots[0].0,
                slots[1].0,
                slots[2].0,
                slots[3].0,
                slots[4].0,
                slots[5].0,
                self.receipts.as_ptr() as usize,
                self.operations.as_ptr() as usize,
                self.resources.as_ptr() as usize,
                parser_address,
            ],
            byte_capacities,
            receipt_capacity_bytes,
            operations_capacity_bytes,
            resource_capacity_bytes,
            lexical_capacity_bytes,
            owned_bytes: size_of::<Self>()
                + byte_capacities.iter().sum::<usize>()
                + receipt_capacity_bytes
                + operations_capacity_bytes
                + resource_capacity_bytes
                + lexical_capacity_bytes,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn strict_bytes_reject_before_extra_decode_and_writer_failure_preserves_backing() {
        let mut b = Bytes::new(4).unwrap();
        let id = b.identity();
        for bad in [
            "[256]",
            "[-1]",
            "[1.5]",
            "[\"1\"]",
            "[0,0,0,0,{\"not_a_byte\":[1,2,3]}]",
        ] {
            let mut d = serde_json::Deserializer::from_str(bad);
            assert!(ByteSeed(&mut b, 4).deserialize(&mut d).is_err());
            assert_eq!(b.identity(), id);
        }
        let mut d = serde_json::Deserializer::from_str("[0,255,42,7]");
        ByteSeed(&mut b, 4).deserialize(&mut d).unwrap();
        d.end().unwrap();
        assert_eq!(b.as_slice(), [0, 255, 42, 7]);
        assert!(b
            .write_with(2, |out| out.write_all(&[1, 2, 3]).map_err(Error::from))
            .is_err());
        assert!(b.as_slice().is_empty());
        b.write_with(2, |out| out.write_all(&[1, 2]).map_err(Error::from))
            .unwrap();
        assert_eq!(b.as_slice(), [1, 2]);
        assert_eq!(b.identity(), id);
    }
    #[test]
    fn metadata_counts_full_receipts_and_all_retained_file_slots() {
        let m = MetadataWorkspace::new().unwrap();
        let id = m.identity();
        assert_eq!(
            id.byte_capacities,
            [
                16 * 1024 * 1024,
                4 * 1024 * 1024,
                32768,
                65536,
                65536,
                65536
            ]
        );
        assert_eq!(
            id.receipt_capacity_bytes,
            MAX_RECEIPTS * size_of::<JournalRecord>()
        );
        assert_eq!(id.resource_capacity_bytes, 8 * size_of::<Resource>());
        assert_eq!(
            id.owned_bytes,
            size_of::<MetadataWorkspace>()
                + id.byte_capacities.iter().sum::<usize>()
                + id.receipt_capacity_bytes
                + id.operations_capacity_bytes
                + id.resource_capacity_bytes
                + id.lexical_capacity_bytes
        );
        assert_eq!(
            id.owned_bytes - size_of::<MetadataWorkspace>(),
            MetadataWorkspace::planned_heap_bytes()
        );
        assert!(id.backings.iter().all(|p| *p != 0));
    }
}
