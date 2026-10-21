//! Fixed diagnostic L1 shape only; never a general World/Arena restore path.
use super::*;
use crate::fixed_restore::ExactSlice;
use serde::de::{self, DeserializeSeed, MapAccess, Visitor};
use std::fmt;

#[derive(Clone, Copy, Deserialize)]
#[serde(field_identifier, rename_all = "snake_case")]
enum Key {
    Scope,
    Slots,
    Free,
    Max,
    Live,
}

pub(crate) struct LayoutInto<'a, D: Domain, T: Copy, const N: usize> {
    pub target: &'a mut Arena<D, T>,
}
impl<'de, D: Domain, T: Copy + Deserialize<'de>, const N: usize> DeserializeSeed<'de>
    for LayoutInto<'_, D, T, N>
{
    type Value = ();
    fn deserialize<S: serde::Deserializer<'de>>(self, d: S) -> Result<(), S::Error> {
        d.deserialize_map(self)
    }
}
impl<'de, D: Domain, T: Copy + Deserialize<'de>, const N: usize> Visitor<'de>
    for LayoutInto<'_, D, T, N>
{
    type Value = ();
    fn expecting(&self, f: &mut fmt::Formatter) -> fmt::Result {
        f.write_str("existing initialized fixed diagnostic Arena")
    }
    fn visit_map<A: MapAccess<'de>>(self, mut a: A) -> Result<(), A::Error> {
        let out = self.target;
        if out.slots.capacity() != N
            || out.slots.len() != N
            || out.free.capacity() != N
            || !out.free.is_empty()
        {
            return Err(de::Error::custom("native_inplace_arena_backing"));
        }
        let mut seen = 0u8;
        while let Some(key) = a.next_key::<Key>()? {
            let bit = 1 << key as u8;
            if seen & bit != 0 {
                return Err(de::Error::custom("native_inplace_arena_duplicate"));
            }
            seen |= bit;
            match key {
                Key::Scope => out.scope = a.next_value()?,
                Key::Slots => a.next_value_seed(ExactSlice {
                    target: &mut out.slots,
                })?,
                Key::Free => a.next_value_seed(ExactSlice {
                    target: &mut out.free,
                })?,
                Key::Max => out.max = a.next_value()?,
                Key::Live => out.live = a.next_value()?,
            }
        }
        if seen != 31
            || out.scope.world() != 0x5441
            || out.scope.branch() != 1
            || out.max as usize != N
            || out.live != N
            || out
                .slots
                .iter()
                .any(|s| s.generation == 0 || s.value.is_none())
        {
            return Err(de::Error::custom("native_fixed_arena_shape"));
        }
        Ok(())
    }
}
