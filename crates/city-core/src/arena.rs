//! Fixed-record, counted storage. A domain has exactly one table in a logical world.
use serde::{Deserialize, Serialize};
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
}
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct InventoryDomain;
impl Domain for InventoryDomain {
    const ID: u16 = 2;
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Handle<D: Domain> {
    raw: RawHandle,
    marker: PhantomData<D>,
}
impl<D: Domain> Handle<D> {
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

#[derive(Clone, Copy, Debug)]
struct Slot<T: Copy> {
    generation: u32,
    value: Option<T>,
}

/// Owns only fixed-size records: T must not hide owned heap storage or borrowed world state.
pub struct Arena<D: Domain, T: Copy> {
    scope: Scope,
    slots: Vec<Slot<T>>,
    free: Vec<u32>,
    max: u32,
    live: usize,
    marker: PhantomData<D>,
}

impl<D: Domain, T: Copy> Arena<D, T> {
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
            if self.slots.len() >= self.max as usize {
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
            self.free.push(handle.raw.index);
        } // Exhausted generation is retired; never wrap to an old live identity.
        Ok(value)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
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
