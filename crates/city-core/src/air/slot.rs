//! Small internal Background profile, distinct from the general 20k-source Air API.
use super::*;
use crate::restore_into::Restore;
use std::io::Read;

#[derive(Debug, PartialEq, Eq, Serialize)]
pub struct StateSlotIdentity {
    pub backing: [usize; 8],
    pub owned_bytes: usize,
}
/// A prebuilt 128² / background::SOURCE_LIMIT-source candidate. Not an admission or broker grant.
/// Failed parses invalidate the entire candidate without changing its backing.
pub struct BackgroundStateSlot {
    schema: String,
    sha256: String,
    payload: State,
    valid: bool,
}
impl Default for BackgroundStateSlot {
    fn default() -> Self {
        Self::new()
    }
}
impl BackgroundStateSlot {
    pub fn new() -> Self {
        Self {
            schema: STRATEGY.into(),
            sha256: "0".repeat(64),
            payload: State::background_empty(),
            valid: false,
        }
    }
    pub fn owned_bytes(&self) -> usize {
        size_of::<Self>() + self.payload.owned_bytes() - size_of::<State>()
            + self.schema.capacity()
            + self.sha256.capacity()
    }
    pub fn identity(&self) -> StateSlotIdentity {
        let b = self.payload.background_allocations();
        StateSlotIdentity {
            backing: [
                b[0],
                b[1],
                b[2],
                b[3],
                b[4],
                b[5],
                self.schema.as_ptr() as usize,
                self.sha256.as_ptr() as usize,
            ],
            owned_bytes: self.owned_bytes(),
        }
    }
    pub fn reset_preserving_capacity(&mut self) {
        self.valid = false;
        self.payload.background_reset();
        self.schema.clear();
        self.schema.push_str(STRATEGY);
        self.sha256.clear();
        self.sha256.extend(std::iter::repeat('0').take(64));
    }
    pub fn state(&self) -> Result<&State, String> {
        if self.valid {
            Ok(&self.payload)
        } else {
            Err("unpublished_air_slot".into())
        }
    }
    /// Advance only the existing payload; never lend mutable ownership of its root.
    pub fn page(&mut self, source_limit: u32, cell_limit: u32) -> Result<(), String> {
        self.state()?;
        let result = self.payload.page(source_limit, cell_limit);
        if result.is_err() {
            self.reset_preserving_capacity();
        }
        result
    }
    pub fn restore_into(&mut self, bytes: &[u8]) -> Result<(), String> {
        self.restore_reader_into(bytes)
    }
    pub fn restore_reader_into(&mut self, reader: impl Read) -> Result<(), String> {
        self.reset_preserving_capacity();
        struct Limited<R> {
            reader: R,
            bytes: usize,
        }
        impl<R: Read> Read for Limited<R> {
            fn read(&mut self, out: &mut [u8]) -> io::Result<usize> {
                if out.is_empty() {
                    return Ok(0);
                }
                let cap = crate::background::ASSET_LIMIT;
                if self.bytes > cap {
                    return Err(io::Error::other("background_asset_bytes"));
                }
                let n = out.len().min(cap + 1 - self.bytes);
                let n = self.reader.read(&mut out[..n])?;
                self.bytes += n;
                if self.bytes > cap {
                    return Err(io::Error::other("background_asset_bytes"));
                }
                Ok(n)
            }
        }
        let result = (|| {
            let reader = crate::fixed_restore::JsonReader::with_string_limit(
                Limited { reader, bytes: 0 },
                384,
            );
            let mut d = serde_json::Deserializer::from_reader(reader);
            self.restore(&mut d).map_err(|e| e.to_string())?;
            d.end().map_err(|e| e.to_string())?;
            if self.schema != STRATEGY || self.sha256 != payload_hash(&self.payload)? {
                return Err("air_checkpoint_checksum".into());
            }
            self.payload.validate()
        })();
        self.finish(result)
    }
    pub(crate) fn begin_into(
        &mut self,
        base: Option<&State>,
        input: &Input,
        expected: [u8; 32],
    ) -> Result<(), String> {
        self.reset_preserving_capacity();
        let result = (|| {
            self.payload.background_begin(base, input)?;
            if self.payload.checkpoint_digest()?.1 != expected {
                return Err("background_input_checksum".into());
            }
            Ok(())
        })();
        self.finish(result)
    }
    fn finish(&mut self, result: Result<(), String>) -> Result<(), String> {
        match result {
            Ok(()) => {
                self.valid = true;
                Ok(())
            }
            Err(e) => {
                self.reset_preserving_capacity();
                Err(e)
            }
        }
    }
}
impl Restore for BackgroundStateSlot {
    fn restore<'de, D: serde::Deserializer<'de>>(&mut self, d: D) -> Result<(), D::Error> {
        crate::restore_into::object!(d,self,BackgroundStateSlot,7u64,{
            schema,0=>Text,sha256,1=>Text,payload,2=>Into,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    fn input(n: u32) -> Input {
        let mut i = Input::background_empty();
        i.target = 60;
        for id in 1..=n {
            i.background_push(Source {
                id,
                cell: id,
                start: 0,
                end: 60,
                rate_micro: 100,
            })
            .unwrap();
        }
        i
    }
    #[test]
    fn same_slot_partial_complete_bad_good_and_general_profile_boundary() {
        let mut slot = BackgroundStateSlot::new();
        let identity = slot.identity();
        assert!(slot.state().is_err());
        for sources in [0, crate::background::SOURCE_LIMIT as u32, 1, 0] {
            let mut old = State::new(input(sources)).unwrap();
            old.page(1, 1).unwrap();
            for complete in [false, true] {
                if complete {
                    while !old.complete() {
                        old.page(8192, 1024).unwrap();
                    }
                }
                let bytes = old.checkpoint().unwrap();
                struct Fragments<'a>(&'a [u8]);
                impl Read for Fragments<'_> {
                    fn read(&mut self, out: &mut [u8]) -> io::Result<usize> {
                        let n = out.len().min(17).min(self.0.len());
                        out[..n].copy_from_slice(&self.0[..n]);
                        self.0 = &self.0[n..];
                        Ok(n)
                    }
                }
                slot.restore_reader_into(Fragments(&bytes)).unwrap();
                assert_eq!(slot.state().unwrap(), &State::restore(&bytes).unwrap());
                assert_eq!(slot.state().unwrap().checkpoint().unwrap(), bytes);
                assert_eq!(
                    slot.state().unwrap().checkpoint_sha256().unwrap(),
                    crate::sha256(&bytes)
                );
                assert_eq!(slot.identity(), identity);
                for bad in [&bytes[..bytes.len() - 1], b"{}", b"null"] {
                    assert!(slot.restore_into(bad).is_err());
                    assert!(slot.state().is_err());
                    assert_eq!(slot.identity(), identity);
                    slot.restore_into(&bytes).unwrap();
                }
            }
        }
        // General API retains its larger source capacity; only this slot is small.
        let mut large = input(crate::background::SOURCE_LIMIT as u32).clone();
        large
            .push_source(Source {
                id: crate::background::SOURCE_LIMIT as u32 + 1,
                cell: 17,
                start: 0,
                end: 60,
                rate_micro: 100,
            })
            .unwrap();
        let bytes = State::new(large).unwrap().checkpoint().unwrap();
        assert!(State::restore(&bytes).is_ok());
        assert!(slot.restore_into(&bytes).is_err());
        assert!(slot.state().is_err());
        assert_eq!(slot.identity(), identity);
    }
    #[test]
    fn checksum_schema_unknown_duplicate_tail_and_limits_cannot_publish() {
        let bytes = State::new(input(1)).unwrap().checkpoint().unwrap();
        let text = String::from_utf8(bytes.clone()).unwrap();
        let mut slot = BackgroundStateSlot::new();
        let identity = slot.identity();
        let mut checksum: serde_json::Value = serde_json::from_slice(&bytes).unwrap();
        checksum["sha256"] = "0".repeat(64).into();
        let mut invalid = vec![
            serde_json::to_vec(&checksum).unwrap(),
            text.replacen(STRATEGY, "unknown", 1).into_bytes(),
            format!("{text} null").into_bytes(),
            format!("{{\"schema\":\"{STRATEGY}\",{}", &text[1..]).into_bytes(),
            text.replacen("{", "{\"unknown\":0,", 1).into_bytes(),
        ];
        let mut oversized = bytes.clone();
        oversized.resize(crate::background::ASSET_LIMIT + 1, b' ');
        invalid.push(oversized);
        invalid.push(format!("{{\"{}\":null}}", "x".repeat(385)).into_bytes());
        for bad in invalid {
            slot.restore_into(&bytes).unwrap();
            assert!(slot.restore_into(&bad).is_err());
            assert!(slot.state().is_err());
            assert_eq!(slot.identity(), identity);
        }
        slot.restore_into(&bytes).unwrap();
        assert_eq!(slot.identity(), identity);
        assert!(slot.page(0, 1).is_err());
        assert!(slot.state().is_err());
        assert_eq!(slot.identity(), identity);
        slot.restore_into(&bytes).unwrap();
        slot.reset_preserving_capacity();
        assert!(slot.state().is_err());
        assert_eq!(slot.identity(), identity);
    }
}
