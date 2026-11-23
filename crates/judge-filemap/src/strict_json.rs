//! Public tool replies must preserve every object member unambiguously.
use serde::{
    Deserialize, Deserializer,
    de::{Error, MapAccess, SeqAccess, Visitor},
};
use serde_json::Value;
use std::fmt;

struct Member(Value);
impl<'de> Deserialize<'de> for Member {
    fn deserialize<D: Deserializer<'de>>(d: D) -> Result<Self, D::Error> {
        struct Json;
        impl<'de> Visitor<'de> for Json {
            type Value = Member;
            fn expecting(&self, f: &mut fmt::Formatter) -> fmt::Result {
                f.write_str("an unambiguous JSON value")
            }
            fn visit_unit<E: Error>(self) -> Result<Member, E> {
                Ok(Member(Value::Null))
            }
            fn visit_bool<E: Error>(self, v: bool) -> Result<Member, E> {
                Ok(Member(v.into()))
            }
            fn visit_i64<E: Error>(self, v: i64) -> Result<Member, E> {
                Ok(Member(v.into()))
            }
            fn visit_u64<E: Error>(self, v: u64) -> Result<Member, E> {
                Ok(Member(v.into()))
            }
            fn visit_f64<E: Error>(self, v: f64) -> Result<Member, E> {
                serde_json::Number::from_f64(v)
                    .map(|n| Member(Value::Number(n)))
                    .ok_or_else(|| E::custom("nonfinite number"))
            }
            fn visit_str<E: Error>(self, v: &str) -> Result<Member, E> {
                Ok(Member(v.into()))
            }
            fn visit_seq<A: SeqAccess<'de>>(self, mut seq: A) -> Result<Member, A::Error> {
                let mut values = vec![];
                while let Some(Member(v)) = seq.next_element()? {
                    values.push(v);
                }
                Ok(Member(Value::Array(values)))
            }
            fn visit_map<A: MapAccess<'de>>(self, mut map: A) -> Result<Member, A::Error> {
                let mut values = serde_json::Map::new();
                while let Some(key) = map.next_key::<String>()? {
                    if values.contains_key(&key) {
                        return Err(A::Error::custom(format!("duplicate JSON member {key}")));
                    }
                    values.insert(key, map.next_value::<Member>()?.0);
                }
                Ok(Member(Value::Object(values)))
            }
        }
        d.deserialize_any(Json)
    }
}
pub(crate) fn parse(bytes: &[u8]) -> Result<Value, String> {
    serde_json::from_slice::<Member>(bytes)
        .map(|m| m.0)
        .map_err(|e| e.to_string())
}
