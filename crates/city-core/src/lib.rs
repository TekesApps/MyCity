//! Authoritative headless model primitives. No window, network or rendering state.

use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::io::{self, Read};

pub mod air;
pub mod arena;
pub mod assembly;
pub mod asset_cost;
pub mod asset_cost_authority;
pub mod boot;
mod bounded_decode;
pub mod calendar;
pub mod canonical;
pub mod construction;
pub mod diagnostics;
pub mod economy;
pub mod education;
pub mod education_runtime;
pub mod education_authority;
pub mod exchange;
pub mod fixed_restore;
pub mod housing;
mod housing_income;
pub mod housing_runtime_move;
pub mod inbox;
pub mod layout_pressure;
pub mod market_access;
// Root-private shared member exclusion; World/Store integration is a separate gate.
pub(crate) mod member_activity;
pub mod member_authority;
pub mod member_qualification;
pub mod qualification_state;
pub(crate) mod labor_qualification;
pub mod course_completion;
pub mod qualification_authority;
pub mod model_capacity;
pub mod model_rows;
pub mod probes;
pub mod region;
pub mod schedule;
pub mod spatial_geometry_permissions;
pub mod spatial_transport_permissions;
pub mod town_economy;
pub mod town_runtime;
pub mod town_utility;
pub mod transaction;
pub mod transport;
pub mod units;
pub mod water_treatment;
mod water_treatment_authority;
pub mod facility_work;
pub mod facility_authority;
pub mod venue_capacity;
pub mod venue_runtime;
pub mod worker_protocol_observation;

pub const SCHEMA: &str = "mycity-m0-bootstrap-v1";
pub const NUMERIC_STRATEGY: &str = "checked-integer-v1";
pub const INPUT_LIMIT: u64 = 64 * 1024;

#[derive(Clone, Debug, Deserialize, Serialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct Bootstrap {
    #[serde(deserialize_with = "crate::bounded_decode::text64")]
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
    crate::bounded_decode::guard(bytes)?;
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
        struct Label;
        impl serde::de::Visitor<'_> for Label {
            type Value = FrozenLabel;
            fn expecting(&self, f: &mut std::fmt::Formatter) -> std::fmt::Result {
                f.write_str("a known finite model label")
            }
            fn visit_str<E: serde::de::Error>(self, s: &str) -> Result<FrozenLabel, E> {
                match s {
                    "pe0-30d-v1" => Ok(FrozenLabel("pe0-30d-v1")),
                    "regional-fulfilment-v1" => Ok(FrozenLabel("regional-fulfilment-v1")),
                    "regional-fulfilment-scale-v1" => {
                        Ok(FrozenLabel("regional-fulfilment-scale-v1"))
                    }
                    "pboot-cold-scale-v1" => Ok(FrozenLabel("pboot-cold-scale-v1")),
                    "pboot-cold-micro-v1" => Ok(FrozenLabel("pboot-cold-micro-v1")),
                    "pboot-local-development-v1" => Ok(FrozenLabel("pboot-local-development-v1")),
                    "pboot-local-development-v2" => Ok(FrozenLabel("pboot-local-development-v2")),
                    "volatile_only" => Ok(FrozenLabel("volatile_only")),
                    _ => Err(E::custom("unsupported_frozen_label")),
                }
            }
        }
        d.deserialize_str(Label)
    }
}

pub mod timeline;

pub mod background;

mod restore_into;

/// Bounded advisory task eligibility; dispatch remains with the existing owner.
pub mod task_registry;

/// Immutable bounded opening transport into caller-owned retained model backing.
pub mod opening_loader;

pub mod service_delivery_runtime;
pub mod service_delivery_authority;
pub mod service_needs;
pub mod service_needs_authority;

/// Exact, bounded candidates; only continuous_supply writes the original ledger.
#[path = "electric_dispatch/continuous_genset.rs"]
pub mod continuous_genset;
#[path = "utility_network/rational_v3.rs"]
pub mod utility_rational_v3;
pub mod continuous_supply;

pub mod time_capacity;

pub mod payroll_members;
