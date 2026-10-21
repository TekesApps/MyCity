//! Active slices backed by either an ordinary Vec or a fully initialized fixed image.
//! Fixed constructors and writes are private to air's typed workspace.
use serde::Serialize;
use std::{
    fmt,
    iter::FromIterator,
    ops::{Deref, DerefMut},
};

pub struct Rows<T> {
    storage: Vec<T>,
    active: usize,
    fixed: bool,
}
impl<T> From<Vec<T>> for Rows<T> {
    fn from(storage: Vec<T>) -> Self {
        let active = storage.len();
        Self {
            storage,
            active,
            fixed: false,
        }
    }
}
impl<T> FromIterator<T> for Rows<T> {
    fn from_iter<I: IntoIterator<Item = T>>(iter: I) -> Self {
        Vec::from_iter(iter).into()
    }
}
impl<T> Deref for Rows<T> {
    type Target = [T];
    fn deref(&self) -> &[T] {
        &self.storage[..self.active]
    }
}
impl<T> DerefMut for Rows<T> {
    fn deref_mut(&mut self) -> &mut [T] {
        &mut self.storage[..self.active]
    }
}
impl<'a, T> IntoIterator for &'a Rows<T> {
    type Item = &'a T;
    type IntoIter = std::slice::Iter<'a, T>;
    fn into_iter(self) -> Self::IntoIter {
        self.iter()
    }
}
impl<T: Clone> Clone for Rows<T> {
    fn clone(&self) -> Self {
        self.deref().to_vec().into()
    }
}
impl<T: PartialEq> PartialEq for Rows<T> {
    fn eq(&self, other: &Self) -> bool {
        self.deref() == other.deref()
    }
}
impl<T: Eq> Eq for Rows<T> {}
impl<T: fmt::Debug> fmt::Debug for Rows<T> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        self.deref().fmt(f)
    }
}
impl<T: Serialize> Serialize for Rows<T> {
    fn serialize<S: serde::Serializer>(&self, s: S) -> Result<S::Ok, S::Error> {
        self.deref().serialize(s)
    }
}
impl<T> Rows<T> {
    pub fn capacity(&self) -> usize {
        self.storage.capacity()
    }
    pub(super) fn push_owned(&mut self, value: T) -> Result<(), String> {
        if self.fixed {
            return Err("air_fixed_rows_mutation".into());
        }
        self.storage.push(value);
        self.active = self.storage.len();
        Ok(())
    }
    pub(super) fn clear_owned(&mut self) -> Result<(), String> {
        if self.fixed {
            return Err("air_fixed_rows_mutation".into());
        }
        self.storage.clear();
        self.active = 0;
        Ok(())
    }
}
impl<T: Clone> Rows<T> {
    pub(super) fn fixed(count: usize, fill: T) -> Result<Self, String> {
        let mut storage = Vec::new();
        storage
            .try_reserve_exact(count)
            .map_err(|e| e.to_string())?;
        if storage.capacity() != count {
            return Err("air_fixed_rows_capacity".into());
        }
        storage.resize(count, fill);
        Ok(Self {
            storage,
            active: 0,
            fixed: true,
        })
    }
    pub(super) fn reset(&mut self, fill: T) -> Result<(), String> {
        if !self.fixed || self.storage.len() != self.storage.capacity() {
            return Err("air_fixed_rows_shape".into());
        }
        self.storage.fill(fill);
        self.active = 0;
        Ok(())
    }
    pub(super) fn write_next(&mut self, value: T) -> Result<(), String> {
        if !self.fixed || self.active >= self.storage.len() {
            return Err("air_fixed_rows_limit".into());
        }
        self.storage[self.active] = value;
        self.active += 1;
        Ok(())
    }
    pub(super) fn fixed_shape(&self, count: usize) -> bool {
        self.fixed
            && self.storage.len() == count
            && self.capacity() == count
            && self.active <= count
    }
    pub(super) fn tail(&self) -> &[T] {
        &self.storage[self.active..]
    }
    pub(super) fn allocation(&self) -> usize {
        self.storage.as_ptr() as usize
    }
}

impl<T: crate::model_rows::Record + for<'de> serde::Deserialize<'de>> crate::restore_into::Restore
    for Rows<T>
{
    fn restore<'de, D: serde::Deserializer<'de>>(&mut self, d: D) -> Result<(), D::Error> {
        struct Read<'a, T>(&'a mut Rows<T>);
        impl<'de, T: crate::model_rows::Record + serde::Deserialize<'de>> serde::de::Visitor<'de>
            for Read<'_, T>
        {
            type Value = ();
            fn expecting(&self, f: &mut fmt::Formatter) -> fmt::Result {
                f.write_str("existing fixed air rows")
            }
            fn visit_seq<A: serde::de::SeqAccess<'de>>(self, mut a: A) -> Result<(), A::Error> {
                let n = self.0.capacity();
                self.0
                    .reset(T::inactive())
                    .map_err(serde::de::Error::custom)?;
                for _ in 0..n {
                    match a.next_element()? {
                        Some(v) => self.0.write_next(v).map_err(serde::de::Error::custom)?,
                        None => return Ok(()),
                    }
                }
                a.next_element_seed(crate::bounded_decode::RejectExtra("air_rows"))?;
                Ok(())
            }
        }
        d.deserialize_seq(Read(self))
    }
}
