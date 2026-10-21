//! Original genesis wire, opening bytes and resources decoded into caller backing.
use super::*;
use serde::de::{self, DeserializeSeed, MapAccess, SeqAccess, Visitor};
use std::fmt;
#[derive(Deserialize)]
struct Label(#[serde(deserialize_with = "metadata::text::<_,64>")] String);
#[derive(Deserialize)]
struct Rules(#[serde(deserialize_with = "metadata::text::<_,128>")] String);
#[derive(Deserialize)]
#[serde(field_identifier, rename_all = "snake_case")]
enum Field {
    Schema,
    RulesIdentity,
    CheckpointLayout,
    Opening,
    Resources,
    Spatial,
}
pub(super) struct Header {
    pub schema: String,
    pub rules_identity: String,
    pub checkpoint_layout: u8,
    pub spatial: Option<SpatialReference>,
}
struct Seed<'a> {
    opening: &'a mut metadata::Bytes,
    resources: &'a mut Vec<Resource>,
}
impl<'de> DeserializeSeed<'de> for Seed<'_> {
    type Value = Header;
    fn deserialize<D: serde::Deserializer<'de>>(self, d: D) -> Result<Header, D::Error> {
        d.deserialize_struct(
            "Genesis",
            &[
                "schema",
                "rules_identity",
                "checkpoint_layout",
                "opening",
                "resources",
                "spatial",
            ],
            self,
        )
    }
}
impl<'de> Visitor<'de> for Seed<'_> {
    type Value = Header;
    fn expecting(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str("bounded original genesis")
    }
    fn visit_seq<A: SeqAccess<'de>>(self, mut seq: A) -> Result<Header, A::Error> {
        let schema: Label = seq
            .next_element()?
            .ok_or_else(|| de::Error::missing_field("schema"))?;
        let rules: Rules = seq
            .next_element()?
            .ok_or_else(|| de::Error::missing_field("rules_identity"))?;
        let checkpoint_layout = seq.next_element()?.unwrap_or(0);
        seq.next_element_seed(metadata::ByteSeed(self.opening, INPUT_LIMIT as usize))?
            .ok_or_else(|| de::Error::missing_field("opening"))?;
        seq.next_element_seed(metadata::RowsSeed(self.resources))?
            .ok_or_else(|| de::Error::missing_field("resources"))?;
        let spatial = seq.next_element()?.unwrap_or(None);
        seq.next_element_seed(metadata::RejectExtra)?;
        Ok(Header {
            schema: schema.0,
            rules_identity: rules.0,
            checkpoint_layout,
            spatial,
        })
    }
    fn visit_map<A: MapAccess<'de>>(self, mut map: A) -> Result<Header, A::Error> {
        let mut seen = [false; 6];
        let mut schema = None;
        let mut rules = None;
        let mut layout = 0;
        let mut spatial = None;
        while let Some(field) = map.next_key::<Field>()? {
            let (slot, name) = match field {
                Field::Schema => (0, "schema"),
                Field::RulesIdentity => (1, "rules_identity"),
                Field::CheckpointLayout => (2, "checkpoint_layout"),
                Field::Opening => (3, "opening"),
                Field::Resources => (4, "resources"),
                Field::Spatial => (5, "spatial"),
            };
            if seen[slot] {
                return Err(de::Error::duplicate_field(name));
            }
            seen[slot] = true;
            match field {
                Field::Schema => schema = Some(map.next_value::<Label>()?.0),
                Field::RulesIdentity => rules = Some(map.next_value::<Rules>()?.0),
                Field::CheckpointLayout => layout = map.next_value()?,
                Field::Opening => {
                    map.next_value_seed(metadata::ByteSeed(self.opening, INPUT_LIMIT as usize))?
                }
                Field::Resources => map.next_value_seed(metadata::RowsSeed(self.resources))?,
                Field::Spatial => spatial = map.next_value()?,
            }
        }
        if !seen[3] || !seen[4] {
            return Err(de::Error::custom("genesis_missing_data"));
        }
        Ok(Header {
            schema: schema.ok_or_else(|| de::Error::missing_field("schema"))?,
            rules_identity: rules.ok_or_else(|| de::Error::missing_field("rules_identity"))?,
            checkpoint_layout: layout,
            spatial,
        })
    }
}
pub(super) fn decode(meta: &mut MetadataWorkspace) -> Result<Header, Error> {
    metadata::guard(meta.file.as_slice(), 768)?;
    let mut d = serde_json::Deserializer::from_slice(meta.file.as_slice());
    let header = Seed {
        opening: &mut meta.opening,
        resources: &mut meta.resources,
    }
    .deserialize(&mut d)?;
    d.end()?;
    Ok(header)
}
