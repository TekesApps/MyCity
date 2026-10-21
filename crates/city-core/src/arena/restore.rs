//! General (including sparse/free/retired) Arena restore. L1 has a distinct shape decoder.
use super::*;
use crate::bounded_decode::VectorSeed;
use serde::de::{self, MapAccess, Visitor};
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
impl<'de, D: Domain, T: Copy + Deserialize<'de>> Deserialize<'de> for Arena<D, T> {
    fn deserialize<S: serde::Deserializer<'de>>(s: S) -> Result<Self, S::Error> {
        struct ReadArena<D: Domain, T: Copy>(PhantomData<(D, T)>);
        impl<'de, D: Domain, T: Copy + Deserialize<'de>> Visitor<'de> for ReadArena<D, T> {
            type Value = Arena<D, T>;
            fn expecting(&self, f: &mut fmt::Formatter) -> fmt::Result {
                f.write_str("trusted-capacity general Arena")
            }
            fn visit_map<A: MapAccess<'de>>(self, mut a: A) -> Result<Self::Value, A::Error> {
                let n = D::RESTORE_LIMIT;
                if n == 0 {
                    return Err(de::Error::custom("arena_restore_domain"));
                }
                let mut out = Arena::new(
                    Scope::new(1, 1).expect("internal scope"),
                    n as u32,
                    crate::assembly::WORLD_BYTE_BUDGET / 2,
                )
                .map_err(|e| de::Error::custom(format!("arena_backing:{e:?}")))?;
                let mut mask = 0u8;
                while let Some(key) = a.next_key::<Key>()? {
                    let bit = 1 << (key as u8);
                    if mask & bit != 0 {
                        return Err(de::Error::custom("arena_duplicate_field"));
                    }
                    mask |= bit;
                    match key {
                        Key::Scope => out.scope = a.next_value()?,
                        Key::Slots => a.next_value_seed(VectorSeed {
                            target: &mut out.slots,
                            limit: n,
                        })?,
                        Key::Free => a.next_value_seed(VectorSeed {
                            target: &mut out.free,
                            limit: n,
                        })?,
                        Key::Max => {
                            out.max = a.next_value()?;
                            if out.max as usize > n {
                                return Err(de::Error::custom("arena_declared_limit"));
                            }
                        }
                        Key::Live => out.live = a.next_value()?,
                    }
                }
                if mask != 31 {
                    return Err(de::Error::custom("arena_missing_field"));
                }
                out.restore_capacity(out.scope, out.max, crate::assembly::WORLD_BYTE_BUDGET / 2)
                    .map_err(|e| de::Error::custom(format!("arena_restore:{e:?}")))?;
                Ok(out)
            }
        }
        s.deserialize_map(ReadArena::<D, T>(PhantomData))
    }
}
