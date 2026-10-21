//! Fixed typed backing with an independent active length. No capacity growth.
use serde::{
    de::{self, DeserializeSeed, SeqAccess, Visitor},
    Deserialize, Deserializer, Serialize, Serializer,
};
use std::{
    fmt,
    mem::size_of,
    ops::{Deref, DerefMut},
};

/// Inactive values are private storage fill, never serialized or business records.
pub trait Record: Copy {
    fn inactive() -> Self;
}

#[derive(Debug)]
pub struct Rows<T: Record, const N: usize> {
    backing: Vec<T>,
    active: usize,
}
impl<T: Record, const N: usize> Rows<T, N> {
    pub fn new() -> Result<Self, &'static str> {
        let bytes = N.checked_mul(size_of::<T>()).ok_or("model_rows_size")?;
        if size_of::<T>() == 0 || bytes > 64 * 1024 * 1024 {
            return Err("model_rows_budget_before_allocation");
        }
        let mut backing = Vec::new();
        backing
            .try_reserve_exact(N)
            .map_err(|_| "model_rows_allocation")?;
        if backing.capacity() != N {
            return Err("model_rows_capacity");
        }
        backing.resize(N, T::inactive());
        Ok(Self { backing, active: 0 })
    }
    pub fn capacity(&self) -> usize {
        N
    }
    pub fn try_reserve_exact(&mut self, additional: usize) -> Result<(), &'static str> {
        if self.active.checked_add(additional).is_some_and(|v| v <= N) {
            Ok(())
        } else {
            Err("model_rows_full")
        }
    }
    pub fn try_push(&mut self, value: T) -> Result<(), &'static str> {
        self.try_reserve_exact(1)?;
        self.backing[self.active] = value;
        self.active += 1;
        Ok(())
    }
    /// Vec-compatible internal callers must check business limits before mutation.
    pub fn push(&mut self, value: T) {
        self.try_push(value)
            .expect("prevalidated fixed row capacity");
    }
    pub fn clear(&mut self) {
        self.backing[..self.active].fill(T::inactive());
        self.active = 0;
    }
    pub fn resize(&mut self, length: usize, fill: T) {
        assert!(length <= N, "prevalidated fixed row length");
        if length > self.active {
            self.backing[self.active..length].fill(fill);
        } else {
            self.backing[length..self.active].fill(T::inactive());
        }
        self.active = length;
    }
    pub fn copy_active_from(&mut self, other: &Self) {
        self.clear();
        self.backing[..other.active].copy_from_slice(other);
        self.active = other.active;
    }
    pub fn allocation(&self) -> usize {
        self.backing.as_ptr() as usize
    }
    pub fn full_fill_valid(&self) -> bool
    where
        T: PartialEq,
    {
        self.backing.len() == N
            && self.backing.capacity() == N
            && self.backing[self.active..]
                .iter()
                .all(|v| *v == T::inactive())
    }
}
impl<T: Record, const N: usize> Clone for Rows<T, N> {
    fn clone(&self) -> Self {
        let mut out = Self::new().expect("fixed row allocation");
        out.copy_active_from(self);
        out
    }
    fn clone_from(&mut self, other: &Self) {
        self.copy_active_from(other);
    }
}
impl<T: Record + PartialEq, const N: usize> PartialEq for Rows<T, N> {
    fn eq(&self, other: &Self) -> bool {
        **self == **other
    }
}
impl<T: Record + Eq, const N: usize> Eq for Rows<T, N> {}
impl<T: Record, const N: usize> Deref for Rows<T, N> {
    type Target = [T];
    fn deref(&self) -> &[T] {
        &self.backing[..self.active]
    }
}
impl<T: Record, const N: usize> DerefMut for Rows<T, N> {
    fn deref_mut(&mut self) -> &mut [T] {
        &mut self.backing[..self.active]
    }
}
impl<'a, T: Record, const N: usize> IntoIterator for &'a Rows<T, N> {
    type Item = &'a T;
    type IntoIter = std::slice::Iter<'a, T>;
    fn into_iter(self) -> Self::IntoIter {
        self.iter()
    }
}
impl<'a, T: Record, const N: usize> IntoIterator for &'a mut Rows<T, N> {
    type Item = &'a mut T;
    type IntoIter = std::slice::IterMut<'a, T>;
    fn into_iter(self) -> Self::IntoIter {
        self.iter_mut()
    }
}
impl<T: Record + Serialize, const N: usize> Serialize for Rows<T, N> {
    fn serialize<S: Serializer>(&self, s: S) -> Result<S::Ok, S::Error> {
        self.deref().serialize(s)
    }
}
struct RejectExtra;
impl<'de> DeserializeSeed<'de> for RejectExtra {
    type Value = ();
    fn deserialize<D: Deserializer<'de>>(self, _: D) -> Result<(), D::Error> {
        Err(de::Error::custom("model_rows_extra_record"))
    }
}
impl<'de, T: Record + Deserialize<'de>, const N: usize> Deserialize<'de> for Rows<T, N> {
    fn deserialize<D: Deserializer<'de>>(d: D) -> Result<Self, D::Error> {
        struct ReadRows<T: Record, const N: usize>(std::marker::PhantomData<T>);
        impl<'de, T: Record + Deserialize<'de>, const N: usize> Visitor<'de> for ReadRows<T, N> {
            type Value = Rows<T, N>;
            fn expecting(&self, f: &mut fmt::Formatter) -> fmt::Result {
                write!(f, "at most {N} model records")
            }
            fn visit_seq<A: SeqAccess<'de>>(self, mut seq: A) -> Result<Self::Value, A::Error> {
                let mut out = Rows::new().map_err(de::Error::custom)?;
                for _ in 0..N {
                    match seq.next_element()? {
                        Some(v) => out.try_push(v).map_err(de::Error::custom)?,
                        None => return Ok(out),
                    }
                }
                seq.next_element_seed(RejectExtra)?;
                Ok(out)
            }
        }
        d.deserialize_seq(ReadRows::<T, N>(std::marker::PhantomData))
    }
}

impl<T: Record + for<'de> Deserialize<'de>, const N: usize> crate::restore_into::Restore
    for Rows<T, N>
{
    fn restore<'de, D: Deserializer<'de>>(&mut self, d: D) -> Result<(), D::Error> {
        struct Read<'a, T: Record, const N: usize>(&'a mut Rows<T, N>);
        impl<'de, T: Record + Deserialize<'de>, const N: usize> Visitor<'de> for Read<'_, T, N> {
            type Value = ();
            fn expecting(&self, f: &mut fmt::Formatter) -> fmt::Result {
                f.write_str("preallocated rows")
            }
            fn visit_seq<A: SeqAccess<'de>>(self, mut a: A) -> Result<(), A::Error> {
                self.0.clear();
                for _ in 0..N {
                    match a.next_element()? {
                        Some(v) => self.0.try_push(v).map_err(de::Error::custom)?,
                        None => return Ok(()),
                    }
                }
                a.next_element_seed(RejectExtra)?;
                Ok(())
            }
        }
        d.deserialize_seq(Read(self))
    }
}

/// Logical null retains its fully allocated storage. No owning Option::take API.
#[derive(Debug)]
pub(crate) struct Resident<T> {
    value: T,
    present: bool,
}
pub(crate) trait ResidentValue: Sized {
    fn empty() -> Self;
    fn reset(&mut self);
}
impl<T: ResidentValue> Resident<T> {
    pub fn empty() -> Self {
        Self {
            value: T::empty(),
            present: false,
        }
    }
    pub fn as_ref(&self) -> Option<&T> {
        self.present.then_some(&self.value)
    }
    pub fn as_mut(&mut self) -> Option<&mut T> {
        self.present.then_some(&mut self.value)
    }
    pub fn is_some(&self) -> bool {
        self.present
    }
    pub fn backing(&self) -> &T {
        &self.value
    }
    pub fn activate(&mut self) -> &mut T {
        self.value.reset();
        self.present = true;
        &mut self.value
    }
    pub fn deactivate(&mut self) {
        self.value.reset();
        self.present = false;
    }
}
impl<T: PartialEq + ResidentValue> PartialEq for Resident<T> {
    fn eq(&self, other: &Self) -> bool {
        self.as_ref() == other.as_ref()
    }
}
impl<T: Eq + ResidentValue> Eq for Resident<T> {}
impl<T: Clone> Clone for Resident<T> {
    fn clone(&self) -> Self {
        Self {
            value: self.value.clone(),
            present: self.present,
        }
    }
    fn clone_from(&mut self, other: &Self) {
        self.value.clone_from(&other.value);
        self.present = other.present;
    }
}
impl<T: ResidentValue + Serialize> Serialize for Resident<T> {
    fn serialize<S: Serializer>(&self, s: S) -> Result<S::Ok, S::Error> {
        self.as_ref().serialize(s)
    }
}
impl<'de, T: ResidentValue + Deserialize<'de>> Deserialize<'de> for Resident<T> {
    fn deserialize<D: Deserializer<'de>>(d: D) -> Result<Self, D::Error> {
        struct ReadResident<T>(std::marker::PhantomData<T>);
        impl<'de, T: ResidentValue + Deserialize<'de>> Visitor<'de> for ReadResident<T> {
            type Value = Resident<T>;
            fn expecting(&self, f: &mut fmt::Formatter) -> fmt::Result {
                f.write_str("null or a resident model value")
            }
            fn visit_none<E: de::Error>(self) -> Result<Self::Value, E> {
                Ok(Resident::empty())
            }
            fn visit_unit<E: de::Error>(self) -> Result<Self::Value, E> {
                self.visit_none()
            }
            fn visit_some<D: Deserializer<'de>>(self, d: D) -> Result<Self::Value, D::Error> {
                Ok(Resident {
                    value: T::deserialize(d)?,
                    present: true,
                })
            }
        }
        d.deserialize_option(ReadResident(std::marker::PhantomData))
    }
}

impl<T: ResidentValue + crate::restore_into::Restore> crate::restore_into::Restore for Resident<T> {
    fn restore<'de, D: Deserializer<'de>>(&mut self, d: D) -> Result<(), D::Error> {
        struct Read<'a, T>(&'a mut Resident<T>);
        impl<'de, T: ResidentValue + crate::restore_into::Restore> Visitor<'de> for Read<'_, T> {
            type Value = ();
            fn expecting(&self, f: &mut fmt::Formatter) -> fmt::Result {
                f.write_str("in-place resident")
            }
            fn visit_none<E: de::Error>(self) -> Result<(), E> {
                self.0.deactivate();
                Ok(())
            }
            fn visit_unit<E: de::Error>(self) -> Result<(), E> {
                self.visit_none()
            }
            fn visit_some<D: Deserializer<'de>>(self, d: D) -> Result<(), D::Error> {
                self.0.deactivate();
                self.0.value.restore(d)?;
                self.0.present = true;
                Ok(())
            }
        }
        d.deserialize_option(Read(self))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
    struct Item(u32);
    impl Record for Item {
        fn inactive() -> Self {
            Self(0)
        }
    }
    #[test]
    fn copy_reset_and_shorter_restore_keep_full_backing_and_wire() {
        let mut a: Rows<Item, 4> = serde_json::from_str("[1,2,3]").unwrap();
        let b: Rows<Item, 4> = serde_json::from_str("[9]").unwrap();
        let id = a.allocation();
        a.copy_active_from(&b);
        assert_eq!(a.allocation(), id);
        assert!(a.full_fill_valid());
        assert_eq!(serde_json::to_string(&a).unwrap(), "[9]");
        a.clear();
        assert_eq!(a.allocation(), id);
        assert!(a.full_fill_valid());
        assert_eq!(a.capacity(), 4);
        assert_eq!(serde_json::to_string(&a).unwrap(), "[]");
    }
    #[test]
    fn overflow_rejected_before_decoding_the_extra_value() {
        let error = serde_json::from_str::<Rows<Item, 1>>("[1,{\"deep\":[[[]]]}]")
            .unwrap_err()
            .to_string();
        assert!(error.contains("model_rows_extra_record"), "{error}");
        let mut rows = Rows::<Item, 1>::new().unwrap();
        rows.try_push(Item(1)).unwrap();
        let id = rows.allocation();
        assert!(rows.try_push(Item(2)).is_err());
        assert_eq!(rows.allocation(), id);
        assert_eq!(&*rows, &[Item(1)]);
    }
    #[test]
    fn inactive_timeline_retains_capacity_and_enable_does_not_allocate_rows() {
        let mut a: Resident<crate::timeline::Timeline> = serde_json::from_str("null").unwrap();
        let claims = a.backing().claims.allocation();
        let payroll = a.backing().payroll.allocation();
        assert!(!a.is_some());
        assert_eq!(serde_json::to_string(&a).unwrap(), "null");
        a.activate().reset_at(3600);
        assert_eq!(a.backing().claims.allocation(), claims);
        assert_eq!(a.backing().payroll.allocation(), payroll);
        let mut b: Resident<crate::timeline::Timeline> = serde_json::from_str("null").unwrap();
        let b_claims = b.backing().claims.allocation();
        b.clone_from(&a);
        assert!(b.is_some());
        assert_eq!(b.backing().claims.allocation(), b_claims);
        assert_eq!(
            serde_json::to_string(&a).unwrap(),
            serde_json::to_string(&b).unwrap()
        );
    }
}
