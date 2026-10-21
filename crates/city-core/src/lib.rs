//! Authoritative headless model primitives. No window, network or rendering state.

use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::io::{self, Read};

pub mod air;
pub mod arena;
pub mod assembly;
pub mod boot;
pub mod calendar;
pub mod diagnostics;
pub mod economy;
pub mod exchange;
pub mod fixed_restore;
pub mod inbox;
pub mod layout_pressure;
pub mod probes;
pub mod region;
pub mod schedule;
pub mod transaction;
pub mod units;

pub const SCHEMA: &str = "mycity-m0-bootstrap-v1";
pub const NUMERIC_STRATEGY: &str = "checked-integer-v1";
pub const INPUT_LIMIT: u64 = 64 * 1024;

#[derive(Clone, Debug, Deserialize, Serialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct Bootstrap {
    pub schema: String,
    pub world: u64,
    pub branch: u64,
    pub seed: u64,
    pub laboratory_only: bool,
}

impl Bootstrap {
    pub fn validate(&self) -> Result<(), &'static str> {
        if self.schema != SCHEMA {
            return Err("unsupported_schema");
        }
        if self.world == 0 || self.branch == 0 {
            return Err("zero_identity");
        }
        if !self.laboratory_only {
            return Err("only_laboratory_world_supported");
        }
        Ok(())
    }
}

pub fn sha256(bytes: &[u8]) -> String {
    format!("{:x}", Sha256::digest(bytes))
}

/// Bounded even for streams with no trustworthy length or a concurrently growing file.
pub fn read_bounded(reader: impl Read, limit: u64) -> io::Result<Vec<u8>> {
    let probe = limit
        .checked_add(1)
        .ok_or_else(|| io::Error::new(io::ErrorKind::InvalidInput, "limit_overflow"))?;
    let mut bytes = Vec::new();
    reader.take(probe).read_to_end(&mut bytes)?;
    if bytes.len() as u64 > limit {
        return Err(io::Error::new(
            io::ErrorKind::InvalidData,
            "input_too_large",
        ));
    }
    Ok(bytes)
}

pub fn parse_bootstrap(bytes: &[u8]) -> Result<Bootstrap, String> {
    if bytes.len() as u64 > INPUT_LIMIT {
        return Err("input_too_large".into());
    }
    let input: Bootstrap = serde_json::from_slice(bytes).map_err(|e| e.to_string())?;
    input.validate().map_err(str::to_owned)?;
    Ok(input)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn digest_has_known_reference_value() {
        assert_eq!(
            sha256(b"abc"),
            "ba7816bf8f01cfea414140de5dae2223b00361a396177a9cb410ff61f20015ad"
        );
    }

    #[test]
    fn stream_bound_does_not_depend_on_file_metadata() {
        assert_eq!(read_bounded(&b"abc"[..], 3).unwrap(), b"abc");
        assert!(read_bounded(&b"abcd"[..], 3).is_err());
        assert!(read_bounded(&b""[..], u64::MAX).is_err());
    }

    #[test]
    fn unsupported_or_ambiguous_bootstrap_is_rejected() {
        let valid = include_bytes!("../../../fixtures/m0-bootstrap.json");
        assert!(parse_bootstrap(valid).is_ok());
        let s = std::str::from_utf8(valid).unwrap();
        assert!(parse_bootstrap(s.replace("bootstrap-v1", "bootstrap-v2").as_bytes()).is_err());
        assert!(parse_bootstrap(s.replace("\"world\": 1", "\"world\": 0").as_bytes()).is_err());
        assert!(parse_bootstrap(s.replace("true", "false").as_bytes()).is_err());
        assert!(parse_bootstrap(
            s.replace("\"world\": 1", "\"world\": 1, \"world\": 2")
                .as_bytes()
        )
        .is_err());
        assert!(parse_bootstrap(
            s.replace("\"seed\": 7", "\"seed\": 7, \"balance\": 999")
                .as_bytes()
        )
        .is_err());
    }
}

/// Finite persisted labels; no borrowed input and no leaked heap strings.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize)]
#[serde(transparent)]
pub struct FrozenLabel(pub(crate) &'static str);
impl FrozenLabel {
    pub const fn as_str(self) -> &'static str {
        self.0
    }
}
impl<'de> Deserialize<'de> for FrozenLabel {
    fn deserialize<D: serde::Deserializer<'de>>(d: D) -> Result<Self, D::Error> {
        let s = String::deserialize(d)?;
        match s.as_str() {
            "pe0-30d-v1" => Ok(Self("pe0-30d-v1")),
            "regional-fulfilment-v1" => Ok(Self("regional-fulfilment-v1")),
            "regional-fulfilment-scale-v1" => Ok(Self("regional-fulfilment-scale-v1")),
            "pboot-cold-scale-v1" => Ok(Self("pboot-cold-scale-v1")),
            "pboot-cold-micro-v1" => Ok(Self("pboot-cold-micro-v1")),
            "volatile_only" => Ok(Self("volatile_only")),
            _ => Err(serde::de::Error::custom("unsupported_frozen_label")),
        }
    }
}

pub mod timeline;

pub mod background;
