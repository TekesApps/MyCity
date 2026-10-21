use super::*;
use mycity_core::fixed_restore::ExactSlice;

struct IntoSeed<'a> {
    expected: &'a Table,
    target: &'a mut Table,
}
impl<'de> DeserializeSeed<'de> for IntoSeed<'_> {
    type Value = ();
    fn deserialize<D: Deserializer<'de>>(self, d: D) -> Result<(), D::Error> {
        d.deserialize_map(self)
    }
}
impl<'de> Visitor<'de> for IntoSeed<'_> {
    type Value = ();
    fn expecting(&self, f: &mut fmt::Formatter) -> fmt::Result {
        f.write_str("existing initialized fixed table backing")
    }
    fn visit_map<A: MapAccess<'de>>(self, mut a: A) -> Result<(), A::Error> {
        let expected_kind = match self.expected {
            Table::W64(_) => Kind::W64,
            Table::W128(_) => Kind::W128,
            Table::W256(_) => Kind::W256,
            Table::PathRefs(_) => Kind::PathRefs,
        };
        let mut kind = false;
        let mut records = false;
        while let Some(field) = a.next_key::<Field>()? {
            match field {
                Field::Kind => {
                    if kind {
                        return Err(de::Error::duplicate_field("record_type"));
                    }
                    if a.next_value::<Kind>()? != expected_kind {
                        return Err(de::Error::custom("layout frozen record type"));
                    }
                    kind = true;
                }
                Field::Records => {
                    if records {
                        return Err(de::Error::duplicate_field("records"));
                    }
                    macro_rules! fill {
                        ($src:ident, $dst:ident) => {{
                            let slots = $dst
                                .get_mut(..$src.len())
                                .ok_or_else(|| de::Error::custom("layout_inplace_capacity"))?;
                            a.next_value_seed(ExactSlice { target: slots })?;
                        }};
                    }
                    match (self.expected, &mut *self.target) {
                        (Table::W64(src), Table::W64(dst)) => fill!(src, dst),
                        (Table::W128(src), Table::W128(dst)) => fill!(src, dst),
                        (Table::W256(src), Table::W256(dst)) => fill!(src, dst),
                        (Table::PathRefs(src), Table::PathRefs(dst)) => fill!(src, dst),
                        _ => return Err(de::Error::custom("layout_inplace_kind")),
                    }
                    records = true;
                }
            }
        }
        if !kind || !records {
            return Err(de::Error::custom("layout_inplace_missing"));
        }
        Ok(())
    }
}
pub(in crate::layout_load) fn decode_into(
    reader: impl Read,
    expected: &Table,
    target: &mut Table,
) -> Result<(), String> {
    let mut d = serde_json::Deserializer::from_reader(JsonReader::new(reader));
    IntoSeed { expected, target }
        .deserialize(&mut d)
        .map_err(|e| e.to_string())?;
    d.end().map_err(|e| e.to_string())?;
    let equal = match (expected, target) {
        (Table::W64(src), Table::W64(dst)) => dst.get(..src.len()) == Some(&src[..]),
        (Table::W128(src), Table::W128(dst)) => dst.get(..src.len()) == Some(&src[..]),
        (Table::W256(src), Table::W256(dst)) => dst.get(..src.len()) == Some(&src[..]),
        (Table::PathRefs(src), Table::PathRefs(dst)) => dst.get(..src.len()) == Some(&src[..]),
        _ => false,
    };
    if !equal {
        return Err("layout_stub_restore_changed".into());
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    fn id(table: &Table) -> (usize, usize) {
        match table {
            Table::W64(v) => (v.as_ptr() as usize, v.capacity()),
            Table::W128(v) => (v.as_ptr() as usize, v.capacity()),
            Table::W256(v) => (v.as_ptr() as usize, v.capacity()),
            Table::PathRefs(v) => (v.as_ptr() as usize, v.capacity()),
        }
    }
    #[test]
    fn each_type_reuses_max_backing_across_counts_order_failures_and_retry() {
        for width in [64, 128, 256, 4] {
            let mut target = Table::new(11, width, 1, 1).unwrap();
            let original_id = id(&target);
            for count in [3, 11, 1] {
                let expected = Table::new(count, width, 2, 3).unwrap();
                let bytes = serde_json::to_vec(&expected).unwrap();
                for bad in [
                    b"{}".as_slice(),
                    b"{\"records\":[],\"record_type\":\"PathRefs\"}",
                    b"{\"unknown\":0}",
                ] {
                    assert!(decode_into(bad, &expected, &mut target).is_err());
                    assert_eq!(original_id, id(&target));
                    decode_into(&bytes[..], &expected, &mut target).unwrap();
                }
                let fields: serde_json::Value = serde_json::from_slice(&bytes).unwrap();
                let reversed = format!(
                    "{{\"records\":{},\"record_type\":{}}}",
                    fields["records"], fields["record_type"]
                );
                decode_into(reversed.as_bytes(), &expected, &mut target).unwrap();
                assert_eq!(original_id, id(&target));
            }
            let oversized = Table::new(12, width, 2, 3).unwrap();
            let bytes = serde_json::to_vec(&oversized).unwrap();
            assert!(decode_into(&bytes[..], &oversized, &mut target).is_err());
            assert_eq!(original_id, id(&target));
        }
    }
}
