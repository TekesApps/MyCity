//! Fixed-record, counted storage. A domain has exactly one table in a logical world.
use serde::{Deserialize, Serialize};
mod restore;
use std::{marker::PhantomData, mem::size_of};

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize)]
pub struct Scope {
    world: u64,
    branch: u64,
}

impl Scope {
    pub fn new(world: u64, branch: u64) -> Result<Self, ArenaError> {
        if world == 0 || branch == 0 {
            return Err(ArenaError::InvalidScope);
        }
        Ok(Self { world, branch })
    }
    pub const fn world(self) -> u64 {
        self.world
    }
    pub const fn branch(self) -> u64 {
        self.branch
    }
}

impl<'de> Deserialize<'de> for Scope {
    fn deserialize<D: serde::Deserializer<'de>>(d: D) -> Result<Self, D::Error> {
        #[derive(Deserialize)]
        #[serde(deny_unknown_fields)]
        struct Raw {
            world: u64,
            branch: u64,
        }
        let raw = Raw::deserialize(d)?;
        Self::new(raw.world, raw.branch).map_err(|_| serde::de::Error::custom("invalid_scope"))
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct RawHandle {
    pub world: u64,
    pub branch: u64,
    pub domain: u16,
    pub index: u32,
    pub generation: u32,
}

pub trait Domain: Copy {
    const ID: u16;
    /// Ordinary model restore only. Large L1 capacities use an explicit trusted decoder.
    const RESTORE_LIMIT: usize = 0;
}

#[derive(Clone, Copy, Debug, Serialize, PartialEq, Eq)]
pub struct Layout {
    pub record_bytes: usize,
    pub slot_bytes: usize,
    pub header_bytes: usize,
    pub slots_capacity: usize,
    pub free_index_capacity: usize,
    pub owned_capacity_bytes: usize,
    pub live_records: usize,
}
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct AccountDomain;
impl Domain for AccountDomain {
    const ID: u16 = 1;
    const RESTORE_LIMIT: usize = crate::assembly::MAX_ACCOUNTS;
}
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct InventoryDomain;
impl Domain for InventoryDomain {
    const ID: u16 = 2;
    const RESTORE_LIMIT: usize = crate::assembly::MAX_STOCKS;
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(bound = "", deny_unknown_fields)]
pub struct Handle<D: Domain> {
    raw: RawHandle,
    #[serde(skip)]
    marker: PhantomData<D>,
}
impl<D: Domain> Handle<D> {
    pub(crate) fn inactive() -> Self {
        Self {
            raw: RawHandle {
                world: 0,
                branch: 0,
                domain: D::ID,
                index: u32::MAX,
                generation: 0,
            },
            marker: PhantomData,
        }
    }
    pub const fn raw(self) -> RawHandle {
        self.raw
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ArenaError {
    InvalidScope,
    Budget,
    Full,
    ForeignScope,
    WrongDomain,
    StaleHandle,
    Allocation,
}

#[derive(Clone, Copy, Debug, Serialize, Deserialize)]
struct Slot<T: Copy> {
    generation: u32,
    value: Option<T>,
}

/// Owns only fixed-size records: T must not hide owned heap storage or borrowed world state.
#[derive(Serialize)]
#[serde(bound(serialize = "T: Serialize"))]
pub struct Arena<D: Domain, T: Copy> {
    scope: Scope,
    slots: Vec<Slot<T>>,
    free: Vec<u32>,
    max: u32,
    live: usize,
    #[serde(skip)]
    marker: PhantomData<D>,
}

impl<D: Domain, T: Copy> Arena<D, T> {
    /// Type-derived backing requirement; does not allocate or certify a runtime owner.
    pub(crate) const fn fixed_payload_bytes(max: usize) -> usize {
        max * (size_of::<Slot<T>>() + size_of::<u32>())
    }
    /// Fixed diagnostic snapshot only: all slots live and free list empty.
    /// This must never replace the general World/Arena deserializer.
    pub(crate) fn deserialize_layout<'de, S, const N: usize>(d: S) -> Result<Self, S::Error>
    where
        S: serde::Deserializer<'de>,
        T: Deserialize<'de>,
    {
        #[derive(Deserialize)]
        #[serde(bound(deserialize = "T: Deserialize<'de>"), deny_unknown_fields)]
        struct Raw<T: Copy, const N: usize> {
            scope: Scope,
            #[serde(deserialize_with = "crate::fixed_restore::vector::<_, _, N>")]
            slots: Vec<Slot<T>>,
            #[serde(deserialize_with = "crate::fixed_restore::vector::<_, _, 0>")]
            free: Vec<u32>,
            max: u32,
            live: usize,
        }
        let raw = Raw::<T, N>::deserialize(d)?;
        if raw.scope.world() != 0x5441
            || raw.scope.branch() != 1
            || raw.max as usize != N
            || raw.live != N
            || raw
                .slots
                .iter()
                .any(|s| s.generation == 0 || s.value.is_none())
        {
            return Err(serde::de::Error::custom("native_fixed_arena_shape"));
        }
        let mut free = raw.free;
        free.try_reserve_exact(N)
            .map_err(serde::de::Error::custom)?;
        if free.capacity() != N {
            return Err(serde::de::Error::custom("native_fixed_arena_capacity"));
        }
        Ok(Self {
            scope: raw.scope,
            slots: raw.slots,
            free,
            max: raw.max,
            live: raw.live,
            marker: PhantomData,
        })
    }
    pub(crate) fn layout_restore_bytes(count: usize) -> Option<usize> {
        count.checked_mul(size_of::<Slot<T>>() + size_of::<u32>())
    }
    pub fn new(scope: Scope, max: u32, byte_budget: usize) -> Result<Self, ArenaError> {
        let minimum = (max as usize)
            .checked_mul(size_of::<Slot<T>>() + size_of::<u32>())
            .and_then(|x| x.checked_add(size_of::<Self>()))
            .ok_or(ArenaError::Budget)?;
        if minimum > byte_budget {
            return Err(ArenaError::Budget);
        }
        let mut slots = Vec::new();
        let mut free = Vec::new();
        slots
            .try_reserve_exact(max as usize)
            .map_err(|_| ArenaError::Allocation)?;
        free.try_reserve_exact(max as usize)
            .map_err(|_| ArenaError::Allocation)?;
        if slots.capacity() != max as usize || free.capacity() != max as usize {
            return Err(ArenaError::Allocation);
        }
        // Initialize and observe the allocation before use, without changing Vec/header
        // shape or L1 accounting. Clear leaves no safely borrowable typed inactive tail.
        slots.resize(
            max as usize,
            Slot {
                generation: 0,
                value: None,
            },
        );
        let scan = std::hint::black_box(slots.as_slice())
            .iter()
            .fold(0u64, |a, s| {
                a.wrapping_add(u64::from(s.generation) + u64::from(s.value.is_some()))
            });
        std::hint::black_box(scan);
        slots.clear();
        free.resize(max as usize, 0);
        std::hint::black_box(
            std::hint::black_box(free.as_slice())
                .iter()
                .fold(0u64, |a, v| a.wrapping_add(u64::from(*v))),
        );
        free.clear();
        let table = Self {
            scope,
            slots,
            free,
            max,
            live: 0,
            marker: PhantomData,
        };
        if table.owned_bytes() > byte_budget {
            return Err(ArenaError::Budget);
        }
        Ok(table)
    }
    pub(crate) fn restore_capacity(
        &mut self,
        scope: Scope,
        max: u32,
        budget: usize,
    ) -> Result<(), ArenaError> {
        if self.scope != scope
            || self.max != max
            || self.slots.len() > max as usize
            || self.free.len() > max as usize
            || self.live != self.slots.iter().filter(|s| s.value.is_some()).count()
            || self.slots.iter().any(|s| s.generation == 0)
        {
            return Err(ArenaError::InvalidScope);
        }
        for (i, index) in self.free.iter().enumerate() {
            if self.free[..i].contains(index)
                || self
                    .slots
                    .get(*index as usize)
                    .is_none_or(|s| s.value.is_some())
            {
                return Err(ArenaError::StaleHandle);
            }
        }
        for (i, s) in self.slots.iter().enumerate() {
            if s.value.is_none() && s.generation != u32::MAX && !self.free.contains(&(i as u32)) {
                return Err(ArenaError::StaleHandle);
            }
        }
        if self.slots.capacity() < max as usize
            || self.free.capacity() < max as usize
            || self.owned_bytes() > budget
        {
            return Err(ArenaError::Budget);
        }
        Ok(())
    }
    pub(crate) fn reset_backing(&mut self) {
        self.slots.clear();
        self.free.clear();
        self.scope = Scope::new(1, 1).expect("internal scope");
        self.max = D::RESTORE_LIMIT as u32;
        self.live = 0;
    }
    pub(crate) fn copy_into(&self, target: &mut Self) -> Result<(), String> {
        if target.slots.capacity() != D::RESTORE_LIMIT
            || target.free.capacity() != D::RESTORE_LIMIT
            || self.slots.len() > D::RESTORE_LIMIT
            || self.free.len() > D::RESTORE_LIMIT
        {
            return Err("arena_copy_backing".into());
        }
        target.slots.clear();
        target.free.clear();
        target.slots.extend_from_slice(&self.slots);
        target.free.extend_from_slice(&self.free);
        target.scope = self.scope;
        target.max = self.max;
        target.live = self.live;
        Ok(())
    }
    pub(crate) fn backing_ids(&self) -> [usize; 2] {
        [self.slots.as_ptr() as usize, self.free.as_ptr() as usize]
    }
    pub fn owned_bytes(&self) -> usize {
        size_of::<Self>()
            + self.slots.capacity() * size_of::<Slot<T>>()
            + self.free.capacity() * size_of::<u32>()
    }
    pub fn layout(&self) -> Layout {
        Layout {
            record_bytes: size_of::<T>(),
            slot_bytes: size_of::<Slot<T>>(),
            header_bytes: size_of::<Self>(),
            slots_capacity: self.slots.capacity(),
            free_index_capacity: self.free.capacity(),
            owned_capacity_bytes: self.owned_bytes(),
            live_records: self.live,
        }
    }
    pub fn len(&self) -> usize {
        self.live
    }
    pub fn is_empty(&self) -> bool {
        self.live == 0
    }
    pub fn insert(&mut self, value: T) -> Result<Handle<D>, ArenaError> {
        let index = if let Some(index) = self.free.pop() {
            self.slots[index as usize].value = Some(value);
            index
        } else {
            if self.slots.len() >= self.max as usize || self.slots.len() >= self.slots.capacity() {
                return Err(ArenaError::Full);
            }
            let index = self.slots.len() as u32;
            self.slots.push(Slot {
                generation: 1,
                value: Some(value),
            });
            index
        };
        self.live += 1;
        Ok(Handle {
            raw: RawHandle {
                world: self.scope.world,
                branch: self.scope.branch,
                domain: D::ID,
                index,
                generation: self.slots[index as usize].generation,
            },
            marker: PhantomData,
        })
    }
    pub fn resolve(&self, raw: RawHandle) -> Result<Handle<D>, ArenaError> {
        if raw.world != self.scope.world || raw.branch != self.scope.branch {
            return Err(ArenaError::ForeignScope);
        }
        if raw.domain != D::ID {
            return Err(ArenaError::WrongDomain);
        }
        let slot = self
            .slots
            .get(raw.index as usize)
            .ok_or(ArenaError::StaleHandle)?;
        if slot.value.is_none() || slot.generation != raw.generation {
            return Err(ArenaError::StaleHandle);
        }
        Ok(Handle {
            raw,
            marker: PhantomData,
        })
    }
    pub fn get(&self, handle: Handle<D>) -> Result<&T, ArenaError> {
        self.resolve(handle.raw)?;
        self.slots[handle.raw.index as usize]
            .value
            .as_ref()
            .ok_or(ArenaError::StaleHandle)
    }
    pub fn replace(&mut self, handle: Handle<D>, value: T) -> Result<(), ArenaError> {
        self.resolve(handle.raw)?;
        self.slots[handle.raw.index as usize].value = Some(value);
        Ok(())
    }
    pub fn remove(&mut self, handle: Handle<D>) -> Result<T, ArenaError> {
        self.resolve(handle.raw)?;
        let slot = &mut self.slots[handle.raw.index as usize];
        let value = slot.value.take().ok_or(ArenaError::StaleHandle)?;
        self.live -= 1;
        if let Some(next) = slot.generation.checked_add(1) {
            slot.generation = next;
            assert!(
                self.free.len() < self.free.capacity(),
                "validated bounded free stack"
            );
            self.free.push(handle.raw.index);
        } // Exhausted generation is retired; never wrap to an old live identity.
        Ok(value)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn fixed_diagnostic_arena_rejects_shape_and_preserves_handles() {
        let raw = r#"{"scope":{"world":21569,"branch":1},"slots":[{"generation":1,"value":7},{"generation":2,"value":8}],"free":[],"max":2,"live":2}"#;
        let decode = |s: &str| {
            Arena::<AccountDomain, u64>::deserialize_layout::<_, 2>(
                &mut serde_json::Deserializer::from_str(s),
            )
        };
        let a = decode(raw).unwrap();
        assert_eq!(a.slots.capacity(), 2);
        assert_eq!(a.free.capacity(), 2);
        assert_eq!(serde_json::to_string(&a).unwrap(), raw);
        for bad in [
            raw.replace("21569", "1"),
            raw.replace("\"branch\":1", "\"branch\":2"),
            raw.replace("\"generation\":2", "\"generation\":0"),
            raw.replace("\"value\":8", "\"value\":null"),
            raw.replace("\"free\":[]", "\"free\":[0]"),
            raw.replace("\"max\":2", "\"max\":3"),
            raw.replace("\"live\":2", "\"live\":1"),
        ] {
            assert!(decode(&bad).is_err(), "{bad}");
        }
    }
    fn table() -> Arena<AccountDomain, u64> {
        Arena::new(Scope::new(1, 1).unwrap(), 2, 4096).unwrap()
    }
    #[test]
    fn wrong_world_branch_domain_and_generation_are_rejected() {
        let mut a = table();
        let h = a.insert(42).unwrap();
        for raw in [
            RawHandle {
                world: 2,
                ..h.raw()
            },
            RawHandle {
                branch: 2,
                ..h.raw()
            },
            RawHandle {
                domain: 2,
                ..h.raw()
            },
            RawHandle {
                generation: 0,
                ..h.raw()
            },
            RawHandle {
                index: 999,
                ..h.raw()
            },
        ] {
            assert!(a.resolve(raw).is_err());
        }
        a.remove(h).unwrap();
        let next = a.insert(99).unwrap();
        assert_eq!(h.raw.index, next.raw.index);
        assert!(a.get(h).is_err());
        assert_eq!(*a.get(next).unwrap(), 99);
    }
    #[test]
    fn full_tables_preserve_existing_state_and_owned_capacity() {
        let mut a = table();
        let bytes = a.owned_bytes();
        let x = a.insert(1).unwrap();
        a.insert(2).unwrap();
        assert_eq!(a.insert(3), Err(ArenaError::Full));
        assert_eq!(a.owned_bytes(), bytes);
        assert_eq!(*a.get(x).unwrap(), 1);
        assert_eq!(a.len(), 2);
        assert!(
            Arena::<AccountDomain, u64>::new(Scope::new(1, 1).unwrap(), u32::MAX, 1024).is_err()
        );
        assert!(Scope::new(0, 1).is_err());
    }
    #[test]
    fn generation_exhaustion_retires_slot_instead_of_resurrecting_old_handle() {
        let mut a = Arena::<AccountDomain, u64>::new(Scope::new(1, 1).unwrap(), 1, 4096).unwrap();
        let h = a.insert(1).unwrap();
        a.slots[0].generation = u32::MAX;
        let end = a
            .resolve(RawHandle {
                generation: u32::MAX,
                ..h.raw()
            })
            .unwrap();
        a.remove(end).unwrap();
        assert_eq!(a.insert(2), Err(ArenaError::Full));
        assert!(a.get(h).is_err());
    }
}

#[cfg(test)]
mod checkpoint_tests {
    use super::*;
    #[test]
    fn checkpoint_preserves_free_stack_generation_and_retired_slots() {
        let scope = Scope::new(1, 1).unwrap();
        let mut a = Arena::<AccountDomain, u64>::new(scope, 3, 4096).unwrap();
        let x = a.insert(1).unwrap();
        let y = a.insert(2).unwrap();
        let z = a.insert(3).unwrap();
        a.remove(y).unwrap();
        a.remove(x).unwrap();
        let bytes = serde_json::to_vec(&a).unwrap();
        let mut restored: Arena<AccountDomain, u64> = serde_json::from_slice(&bytes).unwrap();
        restored.restore_capacity(scope, 3, 4096).unwrap();
        assert_eq!(
            a.insert(4).unwrap().raw(),
            restored.insert(4).unwrap().raw()
        );
        assert_eq!(
            a.insert(5).unwrap().raw(),
            restored.insert(5).unwrap().raw()
        );
        assert!(restored.get(x).is_err());
        assert_eq!(*restored.get(z).unwrap(), 3);
        a.slots[z.raw.index as usize].generation = u32::MAX;
        let h = a
            .resolve(RawHandle {
                generation: u32::MAX,
                ..z.raw()
            })
            .unwrap();
        a.remove(h).unwrap();
        let bytes = serde_json::to_vec(&a).unwrap();
        let mut restored: Arena<AccountDomain, u64> = serde_json::from_slice(&bytes).unwrap();
        restored.restore_capacity(scope, 3, 4096).unwrap();
        assert_eq!(restored.insert(9), Err(ArenaError::Full));
    }
    #[test]
    fn general_restore_is_order_independent_bounded_and_never_reallocates_or_expands_max() {
        let scope = Scope::new(1, 1).unwrap();
        let mut original = Arena::<AccountDomain, u64>::new(scope, 3, 4096).unwrap();
        let a = original.insert(1).unwrap();
        let b = original.insert(2).unwrap();
        let c = original.insert(3).unwrap();
        original.remove(b).unwrap();
        original.remove(a).unwrap();
        // Value's sorted keys put max after free; capacity cannot depend on that order.
        let json = serde_json::to_value(&original).unwrap();
        let mut restored: Arena<AccountDomain, u64> = serde_json::from_value(json.clone()).unwrap();
        let ids = (restored.slots.as_ptr(), restored.free.as_ptr());
        assert_eq!(restored.slots.capacity(), AccountDomain::RESTORE_LIMIT);
        assert_eq!(restored.free.capacity(), AccountDomain::RESTORE_LIMIT);
        assert_eq!(
            serde_json::to_vec(&restored).unwrap(),
            serde_json::to_vec(&original).unwrap()
        );
        restored.restore_capacity(scope, 3, 4096).unwrap();
        assert_eq!(ids, (restored.slots.as_ptr(), restored.free.as_ptr()));
        assert_eq!(
            restored.restore_capacity(scope, 3, 512),
            Err(ArenaError::Budget)
        );
        assert_eq!(
            original.insert(4).unwrap().raw(),
            restored.insert(4).unwrap().raw()
        );
        assert_eq!(
            original.insert(5).unwrap().raw(),
            restored.insert(5).unwrap().raw()
        );
        assert_eq!(restored.insert(6), Err(ArenaError::Full));
        assert_eq!(*restored.get(c).unwrap(), 3);
        let mut corrupt = json;
        corrupt["free"] = serde_json::json!([1, 1]);
        assert!(serde_json::from_value::<Arena<AccountDomain, u64>>(corrupt).is_err());
        let rows = vec![r#"{"generation":1,"value":7}"#; AccountDomain::RESTORE_LIMIT].join(",");
        let extra = format!(
            r#"{{"slots":[{rows},{{"value":{{"deep":[[]]}}}}],"free":[],"scope":{{"world":1,"branch":1}},"max":64,"live":64}}"#
        );
        let err = match serde_json::from_str::<Arena<AccountDomain, u64>>(&extra) {
            Ok(_) => panic!("accepted extra slot"),
            Err(e) => e.to_string(),
        };
        assert!(err.contains("model_rows_extra_record"), "{err}");
    }
    #[test]
    fn last_generation_free_survives_reopen_then_exhausted_slot_stays_retired() {
        let scope = Scope::new(1, 1).unwrap();
        let mut a = Arena::<AccountDomain, u64>::new(scope, 1, 4096).unwrap();
        let h = a.insert(7).unwrap();
        a.slots[0].generation = u32::MAX - 1;
        let before_last = a
            .resolve(RawHandle {
                generation: u32::MAX - 1,
                ..h.raw()
            })
            .unwrap();
        a.remove(before_last).unwrap();
        assert_eq!(a.free.as_slice(), &[0]);
        let mut b: Arena<AccountDomain, u64> =
            serde_json::from_slice(&serde_json::to_vec(&a).unwrap()).unwrap();
        b.restore_capacity(scope, 1, 4096).unwrap();
        let last = b.insert(9).unwrap();
        assert_eq!(last.raw().generation, u32::MAX);
        assert!(b.get(before_last).is_err());
        b.remove(last).unwrap();
        assert!(b.free.is_empty());
        let mut c: Arena<AccountDomain, u64> =
            serde_json::from_slice(&serde_json::to_vec(&b).unwrap()).unwrap();
        c.restore_capacity(scope, 1, 4096).unwrap();
        assert_eq!(c.insert(10), Err(ArenaError::Full));
        assert!(c.get(last).is_err());
    }
}
