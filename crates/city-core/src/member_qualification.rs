//! Shared qualification vocabulary and bounded read projection.
//! A read projection is data, never permission to grant rights or hire a worker.
//! The root-owned original transaction writer alone publishes qualification state.
use crate::{arena::Scope,units::GameTime};
use serde::{Deserialize,Serialize};
pub use crate::member_activity::Members as MemberRange;

pub const MAX_READ_ROWS:usize=16;
#[derive(Clone,Copy,Debug,PartialEq,Eq,Serialize,Deserialize)]
pub enum BaseQualification {Driver,Electric,Water,Medical,Nursing,Police,Fire,Teaching}
impl BaseQualification {
    pub fn bit(self)->u8 {1u8<<match self {Self::Driver=>0,Self::Electric=>1,Self::Water=>2,
        Self::Medical=>3,Self::Nursing=>4,Self::Police=>5,Self::Fire=>6,Self::Teaching=>7}}
}
#[derive(Clone,Copy,Debug,PartialEq,Eq,Serialize,Deserialize)]
pub enum Background {General,Vocational,Higher}
#[derive(Clone,Copy,Debug,PartialEq,Eq,Serialize,Deserialize)]
pub enum Skill {Basic,Technical,Professional}
impl Background {pub fn rank(self)->u8 {match self {Self::General=>0,Self::Vocational=>1,Self::Higher=>2}}}
impl Skill {pub fn rank(self)->u8 {match self {Self::Basic=>0,Self::Technical=>1,Self::Professional=>2}}}

/// All fields are resolved from the original current writer; do not manufacture a
/// valid source by parsing an ID or by claiming a caller-provided "verified" bit.
#[derive(Clone,Copy,Debug,PartialEq,Eq,Serialize,Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ReadSource {
    pub scope:Scope,pub population_origin:[u8;32],pub member_version:u32,
    pub root:[u8;32],pub revision:u64,pub as_of:GameTime,
}
#[derive(Clone,Copy,Debug,PartialEq,Eq,Serialize,Deserialize)]
#[serde(deny_unknown_fields)]
pub struct CapabilityRow {
    pub members:MemberRange,
    pub background:Option<Background>,pub skill:Option<Skill>,
    pub base_known:bool,pub base_mask:u8,
    /// Lack of an endorsement provider stays unknown. It is not an empty known set.
    pub endorsements_known:bool,pub endorsement_mask:u64,
}
#[derive(Clone,Copy,Debug,PartialEq,Eq,Serialize,Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ReadSnapshot {
    pub source:ReadSource,pub requested:MemberRange,
    /// Sorted disjoint exact member segments, not averaged capabilities.
    pub rows:[Option<CapabilityRow>;MAX_READ_ROWS],
    /// False includes gaps and bounded projection exhaustion, never real skill shortage.
    pub complete:bool,
}
