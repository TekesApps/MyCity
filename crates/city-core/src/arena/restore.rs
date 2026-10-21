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

impl<D: Domain, T: Copy + for<'de> Deserialize<'de>> crate::restore_into::Restore for Arena<D, T> {
    fn restore<'de, S: serde::Deserializer<'de>>(&mut self, s: S) -> Result<(), S::Error> {
        struct Read<'a, D: Domain, T: Copy>(&'a mut Arena<D, T>);
        impl<'de, D: Domain, T: Copy + Deserialize<'de>> Visitor<'de> for Read<'_, D, T> {
            type Value = ();
            fn expecting(&self, f: &mut fmt::Formatter) -> fmt::Result {
                f.write_str("existing trusted Arena backing")
            }
            fn visit_map<A: MapAccess<'de>>(self, mut a: A) -> Result<(), A::Error> {
                let out = self.0;
                let n = D::RESTORE_LIMIT;
                if n == 0 || out.slots.capacity() != n || out.free.capacity() != n {
                    return Err(de::Error::custom("arena_seed_backing"));
                }
                out.slots.clear();
                out.free.clear();
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
                    .map_err(|e| de::Error::custom(format!("arena_restore:{e:?}")))
            }
        }
        s.deserialize_map(Read(self))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::restore_into::Restore;
    #[test]
    fn inplace_sparse_free_stack_and_last_generation_reuse_trusted_backing() {
        let scope = Scope::new(1, 1).unwrap();
        let mut original = Arena::<AccountDomain, u64>::new(scope, 3, 4096).unwrap();
        let first = original.insert(1).unwrap();
        let second = original.insert(2).unwrap();
        let last = original.insert(3).unwrap();
        original.remove(second).unwrap();
        original.remove(first).unwrap();
        original.slots[last.raw().index as usize].generation = u32::MAX;
        let bytes = serde_json::to_vec(&original).unwrap();
        let mut target =
            Arena::<AccountDomain, u64>::new(scope, AccountDomain::RESTORE_LIMIT as u32, 32768)
                .unwrap();
        let identity = target.backing_ids();
        target
            .restore(&mut serde_json::Deserializer::from_slice(&bytes))
            .unwrap();
        assert_eq!(target.backing_ids(), identity);
        assert_eq!(serde_json::to_vec(&target).unwrap(), bytes);
        assert_eq!(
            target.insert(9).unwrap().raw(),
            original.insert(9).unwrap().raw()
        );
        target.reset_backing();
        assert_eq!(target.backing_ids(), identity);
        target
            .restore(&mut serde_json::Deserializer::from_slice(&bytes))
            .unwrap();
        assert_eq!(target.backing_ids(), identity);
    }
}
