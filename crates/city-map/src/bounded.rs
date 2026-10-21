//! Limits are applied while decoding, before an excess element is materialized.
use serde::{
    de::{self, DeserializeSeed, SeqAccess, Visitor},
    Deserialize, Deserializer,
};
use std::{fmt, marker::PhantomData};
struct RejectExtra;
impl<'de> DeserializeSeed<'de> for RejectExtra {
    type Value = ();
    fn deserialize<D: Deserializer<'de>>(self, _: D) -> Result<(), D::Error> {
        Err(de::Error::custom("spatial row limit"))
    }
}

pub(crate) fn vector<'de, D, T, const N: usize>(d: D) -> Result<Vec<T>, D::Error>
where
    D: Deserializer<'de>,
    T: Deserialize<'de>,
{
    struct Rows<T, const N: usize>(PhantomData<T>);
    impl<'de, T: Deserialize<'de>, const N: usize> Visitor<'de> for Rows<T, N> {
        type Value = Vec<T>;
        fn expecting(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
            write!(f, "at most {N} rows")
        }
        fn visit_seq<A: SeqAccess<'de>>(self, mut a: A) -> Result<Self::Value, A::Error> {
            let mut out = Vec::new();
            out.try_reserve_exact(N).map_err(de::Error::custom)?;
            while out.len() < N {
                match a.next_element()? {
                    Some(v) => out.push(v),
                    None => return Ok(out),
                }
            }
            a.next_element_seed(RejectExtra)?;
            Ok(out)
        }
    }
    d.deserialize_seq(Rows::<T, N>(PhantomData))
}
/// Raw JSON strings are bounded before serde allocates escape scratch. A legal
/// 1024-byte ASCII license may use six JSON bytes for every decoded byte.
pub(crate) fn guard(bytes: &[u8]) -> Result<(), crate::Error> {
    use std::io::Read;
    let mut reader = mycity_core::fixed_restore::JsonReader::with_string_limit(bytes, 6 * 1024);
    let mut window = [0; 4096];
    while reader.read(&mut window).map_err(|_| crate::Error::Limit)? != 0 {}
    Ok(())
}
pub(crate) fn text<'de, D, const N: usize>(d: D) -> Result<String, D::Error>
where
    D: Deserializer<'de>,
{
    struct Text<const N: usize>;
    impl<const N: usize> Visitor<'_> for Text<N> {
        type Value = String;
        fn expecting(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
            write!(f, "at most {N} UTF8 bytes")
        }
        fn visit_str<E: de::Error>(self, s: &str) -> Result<String, E> {
            if s.len() > N {
                return Err(E::custom("spatial text limit"));
            }
            let mut out = String::new();
            out.try_reserve_exact(N).map_err(E::custom)?;
            out.push_str(s);
            Ok(out)
        }
    }
    d.deserialize_str(Text::<N>)
}
