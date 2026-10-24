//! Content-addressed, bounded geometry revision descriptors and Scene inputs.
//!
//! These files are immutable candidate material. They become active only when the Store
//! publishes a validated authorization/catalog update in the same root commit as the Engine.
use crate::{read_file, Error};
use mycity_core::{
    spatial_geometry_permissions::GeometryRevisionDescriptor, transport::OdDemand, INPUT_LIMIT,
};
use serde::{de, Deserialize, Deserializer, Serialize};
use sha2::{Digest, Sha256};
use std::path::Path;

pub const DESCRIPTOR_PREFIX: &str = "spatial-geometry-descriptor-";
pub const SCENE_PREFIX: &str = "spatial-scene-";
pub const DESCRIPTOR_MAX_BYTES: usize = 4096;
pub const ADAPTER_BASIS_SCHEMA: u16 = 1;

fn raw_sha256(bytes: &[u8]) -> [u8; 32] {
    Sha256::digest(bytes).into()
}

fn adapter_demands<'de, D: Deserializer<'de>>(deserializer: D) -> Result<Vec<OdDemand>, D::Error> {
    struct Bounded;
    impl<'de> de::Visitor<'de> for Bounded {
        type Value = Vec<OdDemand>;
        fn expecting(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
            write!(
                formatter,
                "at most {} adapter demands",
                mycity_map::traffic_adapter::MAX_ADAPTER_DEMANDS
            )
        }
        fn visit_seq<A: de::SeqAccess<'de>>(self, mut seq: A) -> Result<Self::Value, A::Error> {
            if seq
                .size_hint()
                .is_some_and(|hint| hint > mycity_map::traffic_adapter::MAX_ADAPTER_DEMANDS)
            {
                return Err(de::Error::invalid_length(
                    mycity_map::traffic_adapter::MAX_ADAPTER_DEMANDS.saturating_add(1),
                    &self,
                ));
            }
            let mut rows = Vec::new();
            for _ in 0..mycity_map::traffic_adapter::MAX_ADAPTER_DEMANDS {
                match seq.next_element()? {
                    Some(row) => {
                        rows.try_reserve_exact(1).map_err(de::Error::custom)?;
                        if rows.capacity() > mycity_map::traffic_adapter::MAX_ADAPTER_DEMANDS {
                            return Err(de::Error::custom("adapter_demand_capacity"));
                        }
                        rows.push(row);
                    }
                    None => return Ok(rows),
                }
            }
            struct Reject;
            impl<'de> de::DeserializeSeed<'de> for Reject {
                type Value = ();
                fn deserialize<D2: Deserializer<'de>>(self, _: D2) -> Result<(), D2::Error> {
                    Err(de::Error::custom("adapter_demand_limit"))
                }
            }
            seq.next_element_seed(Reject)?;
            Ok(rows)
        }
    }
    deserializer.deserialize_seq(Bounded)
}

/// Immutable, host-supplied inputs to the traffic adapter. In particular, speeds are
/// explicit configuration in centimetres per second; they are never inferred from rounded
/// free-flow seconds. `revision` is deliberately not stored: each derivation receives the
/// current profile revision from Store.
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct SpatialGeometryAdapterBasis {
    pub schema: u16,
    pub world_id: u64,
    pub branch_id: u64,
    pub profile_id: u32,
    pub expected_map_id: u32,
    pub city_id: u32,
    pub building_owner_id: u32,
    pub ids: mycity_map::traffic_adapter::AdapterIds,
    pub terminal: mycity_map::traffic_adapter::TerminalGrant,
    pub connector: mycity_map::traffic_adapter::FacilityOperation,
    pub road: mycity_map::traffic_adapter::FacilityOperation,
    pub road_direction: mycity_map::traffic_adapter::RoadDirection,
    #[serde(deserialize_with = "adapter_demands")]
    pub demands: Vec<OdDemand>,
}

impl SpatialGeometryAdapterBasis {
    pub fn validate(&self) -> Result<(), Error> {
        if self.schema != ADAPTER_BASIS_SCHEMA
            || self.world_id == 0
            || self.branch_id == 0
            || self.profile_id == 0
            || self.expected_map_id == 0
            || self.city_id == 0
            || self.building_owner_id == 0
            || self.demands.len() > mycity_map::traffic_adapter::MAX_ADAPTER_DEMANDS
        {
            return Err(Error::Invalid("geometry_adapter_basis_invalid"));
        }
        Ok(())
    }

    pub fn sha256(&self) -> Result<[u8; 32], Error> {
        self.validate()?;
        let bytes = serde_json::to_vec(self)?;
        Ok(Sha256::digest(bytes).into())
    }

    pub fn adapter_input(&self, revision: u64) -> mycity_map::traffic_adapter::AdapterInput<'_> {
        mycity_map::traffic_adapter::AdapterInput {
            expected_map_id: self.expected_map_id,
            city_id: self.city_id,
            revision,
            building_owner_id: self.building_owner_id,
            ids: self.ids,
            terminal: self.terminal,
            connector: self.connector,
            road: self.road,
            road_direction: self.road_direction,
            demands: &self.demands,
        }
    }

    pub fn owned_capacity_bytes(&self) -> usize {
        std::mem::size_of::<Self>() + self.demands.capacity() * std::mem::size_of::<OdDemand>()
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct EncodedDescriptor {
    pub bytes: Vec<u8>,
    pub sha256: [u8; 32],
}

pub fn encode_descriptor(
    descriptor: &GeometryRevisionDescriptor,
) -> Result<EncodedDescriptor, Error> {
    descriptor
        .validate()
        .map_err(|_| Error::Invalid("geometry_descriptor_invalid"))?;
    let bytes =
        serde_json::to_vec(descriptor).map_err(|_| Error::Invalid("geometry_descriptor_encode"))?;
    if bytes.is_empty() || bytes.len() > DESCRIPTOR_MAX_BYTES {
        return Err(Error::Invalid("geometry_descriptor_limit"));
    }
    let sha256 = Sha256::digest(&bytes).into();
    Ok(EncodedDescriptor { bytes, sha256 })
}

pub fn decode_descriptor(
    bytes: &[u8],
    expected_sha256: [u8; 32],
) -> Result<GeometryRevisionDescriptor, Error> {
    if bytes.is_empty()
        || bytes.len() > DESCRIPTOR_MAX_BYTES
        || raw_sha256(bytes) != expected_sha256
    {
        return Err(Error::Invalid("geometry_descriptor_hash"));
    }
    super::metadata::guard(bytes, 4096).map_err(|_| Error::Invalid("geometry_descriptor_json"))?;
    let descriptor: GeometryRevisionDescriptor =
        serde_json::from_slice(bytes).map_err(|_| Error::Invalid("geometry_descriptor_json"))?;
    descriptor
        .validate()
        .map_err(|_| Error::Invalid("geometry_descriptor_invalid"))?;
    let canonical = encode_descriptor(&descriptor)?;
    if canonical.bytes != bytes || canonical.sha256 != expected_sha256 {
        return Err(Error::Invalid("geometry_descriptor_noncanonical"));
    }
    Ok(descriptor)
}

pub fn read_descriptor(
    dir: &Path,
    expected_sha256: [u8; 32],
) -> Result<GeometryRevisionDescriptor, Error> {
    let path = dir.join(format!(
        "{}{}.json",
        DESCRIPTOR_PREFIX,
        super::hex(&expected_sha256)
    ));
    let bytes = read_file(&path, DESCRIPTOR_MAX_BYTES as u64)?;
    decode_descriptor(&bytes, expected_sha256)
}

pub fn stage_descriptor(
    dir: &Path,
    capacity: super::CapacityPolicy,
    encoded: &EncodedDescriptor,
) -> Result<(), Error> {
    let descriptor = decode_descriptor(&encoded.bytes, encoded.sha256)?;
    if encode_descriptor(&descriptor)? != *encoded {
        return Err(Error::Invalid("geometry_descriptor_noncanonical"));
    }
    super::town_transport::admit_immutable_write(
        dir,
        capacity,
        DESCRIPTOR_PREFIX,
        &encoded.bytes,
        &encoded.sha256,
        false,
    )?;
    super::town_transport::write_immutable(
        dir,
        DESCRIPTOR_PREFIX,
        &encoded.bytes,
        &encoded.sha256,
    )?;
    Ok(())
}

pub fn read_scene_input(dir: &Path, expected_sha256: [u8; 32]) -> Result<Vec<u8>, Error> {
    let path = dir.join(format!(
        "{}{}.json",
        SCENE_PREFIX,
        super::hex(&expected_sha256)
    ));
    let bytes = read_file(&path, INPUT_LIMIT)?;
    if bytes.is_empty() || raw_sha256(&bytes) != expected_sha256 {
        return Err(Error::Invalid("geometry_scene_hash"));
    }
    Ok(bytes)
}

pub fn stage_scene_input(
    dir: &Path,
    capacity: super::CapacityPolicy,
    bytes: &[u8],
    expected_sha256: [u8; 32],
) -> Result<(), Error> {
    if bytes.is_empty() || bytes.len() as u64 > INPUT_LIMIT || raw_sha256(bytes) != expected_sha256
    {
        return Err(Error::Invalid("geometry_scene_hash"));
    }
    super::town_transport::admit_immutable_write(
        dir,
        capacity,
        SCENE_PREFIX,
        bytes,
        &expected_sha256,
        false,
    )?;
    super::town_transport::write_immutable(dir, SCENE_PREFIX, bytes, &expected_sha256)?;
    Ok(())
}
