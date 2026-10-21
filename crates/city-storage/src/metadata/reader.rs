//! Sealed typed storage reader. Known value strings are <=128 decoded bytes;
//! schema identifiers are <=64. Unknown fields use the fixed Ignore scanner.
use super::lexical::{self, Cursor, Raw, Scratch};
use serde::{
    de::{self, DeserializeSeed, Visitor},
    Deserialize, Deserializer,
};
use serde_json::Error;

pub(crate) struct Reader<'de, 'work> {
    cursor: Cursor<'de>,
    scratch: &'work mut Scratch,
}
impl<'de, 'work> Reader<'de, 'work> {
    pub fn new(raw: Raw<'de>, scratch: &'work mut Scratch) -> Result<Self, Error> {
        Ok(Self {
            cursor: Cursor::new(raw)?,
            scratch,
        })
    }
    pub fn end(&mut self) -> Result<(), Error> {
        self.cursor.end()
    }
}
pub(crate) fn decode<'de, T: Deserialize<'de>>(
    raw: Raw<'de>,
    scratch: &mut Scratch,
) -> Result<T, Error> {
    seed(raw, std::marker::PhantomData::<T>, scratch)
}
pub(crate) fn seed<'de, S: DeserializeSeed<'de>>(
    raw: Raw<'de>,
    seed: S,
    scratch: &mut Scratch,
) -> Result<S::Value, Error> {
    let mut d = Reader::new(raw, scratch)?;
    let value = seed.deserialize(&mut d)?;
    d.end()?;
    Ok(value)
}
struct Sequence<'a, 'de, 'work> {
    reader: &'a mut Reader<'de, 'work>,
    first: bool,
}
impl<'de> de::SeqAccess<'de> for Sequence<'_, 'de, '_> {
    type Error = Error;
    fn next_element_seed<T: DeserializeSeed<'de>>(
        &mut self,
        seed: T,
    ) -> Result<Option<T::Value>, Error> {
        if self.reader.cursor.peek() == Some(b']') {
            return Ok(None);
        }
        if !self.first {
            self.reader.cursor.eat(b',')?;
        }
        self.first = false;
        seed.deserialize(&mut *self.reader).map(Some)
    }
}
struct Object<'a, 'de, 'work> {
    reader: &'a mut Reader<'de, 'work>,
    first: bool,
    pending: bool,
}
impl<'de> de::MapAccess<'de> for Object<'_, 'de, '_> {
    type Error = Error;
    fn next_key_seed<K: DeserializeSeed<'de>>(
        &mut self,
        seed: K,
    ) -> Result<Option<K::Value>, Error> {
        if self.pending {
            return Err(lexical::error("lexical_value_pending"));
        }
        if self.reader.cursor.peek() == Some(b'}') {
            return Ok(None);
        }
        if !self.first {
            self.reader.cursor.eat(b',')?;
        }
        self.first = false;
        let key = self.reader.cursor.text(true)?;
        self.pending = true;
        seed.deserialize(de::value::StrDeserializer::<Error>::new(
            key.identifier().for_visitor(),
        ))
        .map(Some)
    }
    fn next_value_seed<V: DeserializeSeed<'de>>(&mut self, seed: V) -> Result<V::Value, Error> {
        if !self.pending {
            return Err(lexical::error("lexical_key_required"));
        }
        self.reader.cursor.eat(b':')?;
        self.pending = false;
        seed.deserialize(&mut *self.reader)
    }
}
macro_rules! number_methods{($($name:ident),*$(,)?)=>{$(
    fn $name<V:Visitor<'de>>(self,visitor:V)->Result<V::Value,Error>{
        let raw=self.cursor.number()?;let mut d=serde_json::Deserializer::from_slice(raw);
        let result=Deserializer::$name(&mut d,visitor)?;d.end()?;Ok(result)
    }
)*};}
impl<'de> Deserializer<'de> for &mut Reader<'de, '_> {
    type Error = Error;
    fn deserialize_any<V: Visitor<'de>>(self, visitor: V) -> Result<V::Value, Error> {
        match self.cursor.peek() {
            Some(b'{') => self.deserialize_map(visitor),
            Some(b'[') => self.deserialize_seq(visitor),
            Some(b'"') => self.deserialize_str(visitor),
            Some(b'n') => self.deserialize_unit(visitor),
            Some(b't' | b'f') => self.deserialize_bool(visitor),
            Some(b'-' | b'0'..=b'9') => {
                let raw = self.cursor.number()?;
                let mut d = serde_json::Deserializer::from_slice(raw);
                let result = Deserializer::deserialize_any(&mut d, visitor)?;
                d.end()?;
                Ok(result)
            }
            _ => Err(lexical::error("lexical_value")),
        }
    }
    number_methods!(
        deserialize_i8,
        deserialize_i16,
        deserialize_i32,
        deserialize_i64,
        deserialize_i128,
        deserialize_u8,
        deserialize_u16,
        deserialize_u32,
        deserialize_u64,
        deserialize_u128,
        deserialize_f32,
        deserialize_f64
    );
    fn deserialize_bool<V: Visitor<'de>>(self, visitor: V) -> Result<V::Value, Error> {
        let raw = self.cursor.raw(self.scratch)?;
        match raw.bytes {
            b"true" => visitor.visit_bool(true),
            b"false" => visitor.visit_bool(false),
            _ => Err(lexical::error("lexical_bool")),
        }
    }
    fn deserialize_str<V: Visitor<'de>>(self, visitor: V) -> Result<V::Value, Error> {
        let text = self.cursor.text(true)?;
        visitor.visit_str(text.as_str()?)
    }
    fn deserialize_string<V: Visitor<'de>>(self, visitor: V) -> Result<V::Value, Error> {
        self.deserialize_str(visitor)
    }
    fn deserialize_char<V: Visitor<'de>>(self, visitor: V) -> Result<V::Value, Error> {
        let text = self.cursor.text(true)?;
        let mut chars = text.as_str()?.chars();
        let c = chars.next().ok_or_else(|| lexical::error("lexical_char"))?;
        if chars.next().is_some() {
            return Err(lexical::error("lexical_char"));
        }
        visitor.visit_char(c)
    }
    fn deserialize_bytes<V: Visitor<'de>>(self, visitor: V) -> Result<V::Value, Error> {
        self.deserialize_seq(visitor)
    }
    fn deserialize_byte_buf<V: Visitor<'de>>(self, visitor: V) -> Result<V::Value, Error> {
        self.deserialize_seq(visitor)
    }
    fn deserialize_option<V: Visitor<'de>>(self, visitor: V) -> Result<V::Value, Error> {
        if self.cursor.peek() == Some(b'n') {
            let raw = self.cursor.raw(self.scratch)?;
            if raw.bytes != b"null" {
                return Err(lexical::error("lexical_null"));
            }
            visitor.visit_none()
        } else {
            visitor.visit_some(self)
        }
    }
    fn deserialize_unit<V: Visitor<'de>>(self, visitor: V) -> Result<V::Value, Error> {
        let raw = self.cursor.raw(self.scratch)?;
        if raw.bytes != b"null" {
            return Err(lexical::error("lexical_null"));
        }
        visitor.visit_unit()
    }
    fn deserialize_unit_struct<V: Visitor<'de>>(
        self,
        _: &'static str,
        visitor: V,
    ) -> Result<V::Value, Error> {
        self.deserialize_unit(visitor)
    }
    fn deserialize_newtype_struct<V: Visitor<'de>>(
        self,
        _: &'static str,
        visitor: V,
    ) -> Result<V::Value, Error> {
        visitor.visit_newtype_struct(self)
    }
    fn deserialize_seq<V: Visitor<'de>>(self, visitor: V) -> Result<V::Value, Error> {
        self.cursor.eat(b'[')?;
        self.cursor.enter()?;
        let result = visitor.visit_seq(Sequence {
            reader: self,
            first: true,
        })?;
        self.cursor.eat(b']')?;
        self.cursor.depth -= 1;
        Ok(result)
    }
    fn deserialize_tuple<V: Visitor<'de>>(self, _: usize, visitor: V) -> Result<V::Value, Error> {
        self.deserialize_seq(visitor)
    }
    fn deserialize_tuple_struct<V: Visitor<'de>>(
        self,
        _: &'static str,
        _: usize,
        visitor: V,
    ) -> Result<V::Value, Error> {
        self.deserialize_seq(visitor)
    }
    fn deserialize_map<V: Visitor<'de>>(self, visitor: V) -> Result<V::Value, Error> {
        self.cursor.eat(b'{')?;
        self.cursor.enter()?;
        let result = visitor.visit_map(Object {
            reader: self,
            first: true,
            pending: false,
        })?;
        self.cursor.eat(b'}')?;
        self.cursor.depth -= 1;
        Ok(result)
    }
    fn deserialize_struct<V: Visitor<'de>>(
        self,
        _: &'static str,
        fields: &'static [&'static str],
        visitor: V,
    ) -> Result<V::Value, Error> {
        if fields.iter().any(|s| s.len() > lexical::IDENTIFIER_BYTES) {
            return Err(lexical::error("lexical_schema_identifier"));
        }
        match self.cursor.peek() {
            Some(b'[') => self.deserialize_seq(visitor),
            _ => self.deserialize_map(visitor),
        }
    }
    fn deserialize_identifier<V: Visitor<'de>>(self, visitor: V) -> Result<V::Value, Error> {
        let text = self.cursor.text(true)?;
        visitor.visit_str(text.identifier().for_visitor())
    }
    fn deserialize_enum<V: Visitor<'de>>(
        self,
        _: &'static str,
        variants: &'static [&'static str],
        visitor: V,
    ) -> Result<V::Value, Error> {
        if variants.iter().any(|s| s.len() > lexical::IDENTIFIER_BYTES) {
            return Err(lexical::error("lexical_schema_identifier"));
        }
        if self.cursor.peek() == Some(b'"') {
            let text = self.cursor.text(true)?;
            return visitor.visit_enum(de::value::StrDeserializer::<Error>::new(text.as_str()?));
        }
        self.cursor.eat(b'{')?;
        self.cursor.enter()?;
        let result = visitor.visit_enum(Enum { reader: self })?;
        self.cursor.eat(b'}')?;
        self.cursor.depth -= 1;
        Ok(result)
    }
    fn deserialize_ignored_any<V: Visitor<'de>>(self, visitor: V) -> Result<V::Value, Error> {
        self.cursor.skip(self.scratch, lexical::Mode::Ignore)?;
        visitor.visit_unit()
    }
}
struct Enum<'a, 'de, 'work> {
    reader: &'a mut Reader<'de, 'work>,
}
impl<'de> de::EnumAccess<'de> for Enum<'_, 'de, '_> {
    type Error = Error;
    type Variant = Self;
    fn variant_seed<V: DeserializeSeed<'de>>(self, seed: V) -> Result<(V::Value, Self), Error> {
        let key = self.reader.cursor.text(true)?;
        let value = seed.deserialize(de::value::StrDeserializer::<Error>::new(key.as_str()?))?;
        self.reader.cursor.eat(b':')?;
        Ok((value, self))
    }
}
impl<'de> de::VariantAccess<'de> for Enum<'_, 'de, '_> {
    type Error = Error;
    fn unit_variant(self) -> Result<(), Error> {
        Deserialize::deserialize(&mut *self.reader)
    }
    fn newtype_variant_seed<T: DeserializeSeed<'de>>(self, seed: T) -> Result<T::Value, Error> {
        seed.deserialize(&mut *self.reader)
    }
    fn tuple_variant<V: Visitor<'de>>(self, len: usize, visitor: V) -> Result<V::Value, Error> {
        self.reader.deserialize_tuple(len, visitor)
    }
    fn struct_variant<V: Visitor<'de>>(
        self,
        fields: &'static [&'static str],
        visitor: V,
    ) -> Result<V::Value, Error> {
        self.reader.deserialize_struct("variant", fields, visitor)
    }
}
impl<'de, const N: usize> lexical::Fields<'de, N> {
    pub fn get<T: Deserialize<'de>>(
        &self,
        index: usize,
        scratch: &mut Scratch,
    ) -> Result<T, Error> {
        decode(self.raw(index)?, scratch)
    }
    pub fn optional<T: Deserialize<'de>>(
        &self,
        index: usize,
        scratch: &mut Scratch,
    ) -> Result<Option<T>, Error> {
        match self.values[index] {
            Some(raw) => decode(raw, scratch),
            None => Ok(None),
        }
    }
}
