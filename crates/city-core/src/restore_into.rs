//! In-place seeds for trusted, already initialized model backing. No fresh root.
use serde::{
    de::{self, DeserializeSeed, Visitor},
    Deserialize, Deserializer,
};
use std::fmt;
pub(crate) trait Restore {
    fn restore<'de, D: Deserializer<'de>>(&mut self, d: D) -> Result<(), D::Error>;
}
pub(crate) struct Into<'a, T>(pub &'a mut T);
impl<'de, T: Restore> DeserializeSeed<'de> for Into<'_, T> {
    type Value = ();
    fn deserialize<D: Deserializer<'de>>(self, d: D) -> Result<(), D::Error> {
        self.0.restore(d)
    }
}
pub(crate) struct Copy<'a, T>(pub &'a mut T);
impl<'de, T: std::marker::Copy + Deserialize<'de>> DeserializeSeed<'de> for Copy<'_, T> {
    type Value = ();
    fn deserialize<D: Deserializer<'de>>(self, d: D) -> Result<(), D::Error> {
        *self.0 = T::deserialize(d)?;
        Ok(())
    }
}
/// Sealed model leaves whose deserializer cannot own heap-backed fields.
pub(crate) trait FlatValue {}
pub(crate) struct Flat<'a, T>(pub &'a mut T);
impl<'de, T: FlatValue + Deserialize<'de>> DeserializeSeed<'de> for Flat<'_, T> {
    type Value = ();
    fn deserialize<D: Deserializer<'de>>(self, d: D) -> Result<(), D::Error> {
        *self.0 = T::deserialize(d)?;
        Ok(())
    }
}
impl<T: Restore> Restore for Box<T> {
    fn restore<'de, D: Deserializer<'de>>(&mut self, d: D) -> Result<(), D::Error> {
        self.as_mut().restore(d)
    }
}
pub(crate) struct Text<'a>(pub &'a mut String);
impl Visitor<'_> for Text<'_> {
    type Value = ();
    fn expecting(&self, f: &mut fmt::Formatter) -> fmt::Result {
        f.write_str("existing bounded text backing")
    }
    fn visit_str<E: de::Error>(self, v: &str) -> Result<(), E> {
        if v.len() > 64 || v.len() > self.0.capacity() {
            return Err(E::custom("restore_text_capacity"));
        }
        self.0.clear();
        self.0.push_str(v);
        Ok(())
    }
}
impl<'de> DeserializeSeed<'de> for Text<'_> {
    type Value = ();
    fn deserialize<D: Deserializer<'de>>(self, d: D) -> Result<(), D::Error> {
        d.deserialize_str(self)
    }
}
pub(crate) struct Key {
    bytes: [u8; 64],
    len: usize,
}
impl Key {
    pub fn as_str(&self) -> &str {
        std::str::from_utf8(&self.bytes[..self.len]).expect("decoded UTF8")
    }
}
impl<'de> Deserialize<'de> for Key {
    fn deserialize<D: Deserializer<'de>>(d: D) -> Result<Self, D::Error> {
        struct V;
        impl Visitor<'_> for V {
            type Value = Key;
            fn expecting(&self, f: &mut fmt::Formatter) -> fmt::Result {
                f.write_str("bounded model field")
            }
            fn visit_str<E: de::Error>(self, s: &str) -> Result<Key, E> {
                if s.len() > 64 {
                    return Err(E::custom("restore_field_bound"));
                }
                let mut k = Key {
                    bytes: [0; 64],
                    len: s.len(),
                };
                k.bytes[..s.len()].copy_from_slice(s.as_bytes());
                Ok(k)
            }
        }
        d.deserialize_str(V)
    }
}
macro_rules! object {
 ($decoder:expr,$target:expr,$ty:ty,$required:expr,{$($field:ident,$bit:expr=>$kind:ident),*$(,)?})=>{{
    struct Read<'a>(&'a mut $ty);
    impl<'de> serde::de::Visitor<'de> for Read<'_> {
        type Value=();
        fn expecting(&self,f:&mut std::fmt::Formatter)->std::fmt::Result {f.write_str("in-place model object")}
        fn visit_map<A:serde::de::MapAccess<'de>>(self,mut a:A)->Result<(),A::Error>{
            let value=self.0;let mut seen=0u64;
            while let Some(key)=a.next_key::<crate::restore_into::Key>()? {
                match key.as_str() {$(stringify!($field)=>{
                    let bit=1u64<<$bit;
                    if seen&bit!=0{return Err(serde::de::Error::custom("restore_duplicate_field"));}
                    seen|=bit;a.next_value_seed(crate::restore_into::$kind(&mut value.$field))?;
                }),*,_=>return Err(serde::de::Error::custom("restore_unknown_field"))}
            }
            if seen&$required != $required {return Err(serde::de::Error::custom("restore_missing_field"));}
            Ok(())
        }
    }
    $decoder.deserialize_map(Read($target))
 }};
}
pub(crate) use object;
