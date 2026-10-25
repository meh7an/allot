//! A JSON tree that keeps its objects' keys in file order, for editing files Claude Code owns:
//! what rh does not touch comes back as it was, apart from whitespace.

use std::fmt;

use anyhow::Context;
use serde::{
    Deserialize, Deserializer, Serialize, Serializer,
    de::{self, MapAccess, SeqAccess, Visitor},
    ser::SerializeMap,
};
use serde_json::Number;

/// A JSON value whose objects keep their keys in file order (`serde_json::Value` sorts them).
#[derive(Debug, Clone, PartialEq)]
pub(super) enum Json {
    Null,
    Bool(bool),
    Number(Number),
    String(String),
    Array(Vec<Json>),
    Object(Vec<(String, Json)>),
}

impl Json {
    pub(super) fn object(entries: Vec<(&str, Json)>) -> Self {
        Self::Object(
            entries
                .into_iter()
                .map(|(key, value)| (key.to_owned(), value))
                .collect(),
        )
    }

    pub(super) fn string(value: impl Into<String>) -> Self {
        Self::String(value.into())
    }

    pub(super) fn array_mut(&mut self) -> Option<&mut Vec<Self>> {
        match self {
            Self::Array(items) => Some(items),
            _ => None,
        }
    }

    pub(super) fn object_mut(&mut self) -> Option<&mut Vec<(String, Self)>> {
        match self {
            Self::Object(entries) => Some(entries),
            _ => None,
        }
    }

    pub(super) fn field_mut(&mut self, key: &str) -> Option<&mut Self> {
        self.object_mut()?
            .iter_mut()
            .find(|(name, _)| name == key)
            .map(|(_, value)| value)
    }

    /// The value under `key`, added at the end as `default` when missing; `None` when this is
    /// not an object.
    pub(super) fn entry(&mut self, key: &str, default: Self) -> Option<&mut Self> {
        let entries = self.object_mut()?;
        let index = match entries.iter().position(|(name, _)| name == key) {
            Some(index) => index,
            None => {
                entries.push((key.to_owned(), default));
                entries.len() - 1
            }
        };
        Some(&mut entries[index].1)
    }

    /// Removes `key`; returns whether it was there.
    pub(super) fn remove(&mut self, key: &str) -> bool {
        let Some(entries) = self.object_mut() else {
            return false;
        };
        let before = entries.len();
        entries.retain(|(name, _)| name != key);
        entries.len() != before
    }
}

impl<'de> Deserialize<'de> for Json {
    fn deserialize<D: Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        deserializer.deserialize_any(JsonVisitor)
    }
}

struct JsonVisitor;

impl<'de> Visitor<'de> for JsonVisitor {
    type Value = Json;

    fn expecting(&self, formatter: &mut fmt::Formatter) -> fmt::Result {
        formatter.write_str("a JSON value")
    }

    fn visit_unit<E>(self) -> Result<Json, E> {
        Ok(Json::Null)
    }

    fn visit_bool<E>(self, value: bool) -> Result<Json, E> {
        Ok(Json::Bool(value))
    }

    fn visit_i64<E>(self, value: i64) -> Result<Json, E> {
        Ok(Json::Number(value.into()))
    }

    fn visit_u64<E>(self, value: u64) -> Result<Json, E> {
        Ok(Json::Number(value.into()))
    }

    fn visit_f64<E: de::Error>(self, value: f64) -> Result<Json, E> {
        Number::from_f64(value)
            .map(Json::Number)
            .ok_or_else(|| E::custom("a number that is not finite"))
    }

    fn visit_str<E>(self, value: &str) -> Result<Json, E> {
        Ok(Json::String(value.to_owned()))
    }

    fn visit_string<E>(self, value: String) -> Result<Json, E> {
        Ok(Json::String(value))
    }

    fn visit_seq<A: SeqAccess<'de>>(self, mut seq: A) -> Result<Json, A::Error> {
        let mut items = Vec::new();
        while let Some(item) = seq.next_element()? {
            items.push(item);
        }
        Ok(Json::Array(items))
    }

    fn visit_map<A: MapAccess<'de>>(self, mut map: A) -> Result<Json, A::Error> {
        let mut entries = Vec::new();
        while let Some(entry) = map.next_entry::<String, Json>()? {
            entries.push(entry);
        }
        Ok(Json::Object(entries))
    }
}

impl Serialize for Json {
    fn serialize<S: Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        match self {
            Self::Null => serializer.serialize_unit(),
            Self::Bool(value) => serializer.serialize_bool(*value),
            Self::Number(value) => value.serialize(serializer),
            Self::String(value) => serializer.serialize_str(value),
            Self::Array(items) => serializer.collect_seq(items),
            Self::Object(entries) => {
                let mut map = serializer.serialize_map(Some(entries.len()))?;
                for (key, value) in entries {
                    map.serialize_entry(key, value)?;
                }
                map.end()
            }
        }
    }
}

/// The file's text as a tree; an empty file is an empty object.
///
/// # Errors
///
/// Fails when the text is not JSON.
pub(super) fn parse(text: &str) -> anyhow::Result<Json> {
    if text.trim().is_empty() {
        return Ok(Json::Object(Vec::new()));
    }
    serde_json::from_str(text).context("the file is not valid JSON")
}

/// Pretty JSON with two-space indents, as Claude Code writes it, keeping the original's line
/// endings and final newline.
pub(super) fn text(tree: &Json, original: &str) -> String {
    let mut text = serde_json::to_string_pretty(tree).unwrap_or_default();
    if original.is_empty() || original.ends_with('\n') {
        text.push('\n');
    }
    if original.contains("\r\n") {
        text = text.replace('\n', "\r\n");
    }
    text
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_json_round_trips_values_unchanged() {
        let text = r#"{"b":[1,-2,3.5,true,null,"s"],"a":{"y":{},"x":[]}}"#;
        let tree: Json = serde_json::from_str(text).unwrap();
        assert_eq!(serde_json::to_string(&tree).unwrap(), text);
    }

    #[test]
    fn test_remove_drops_only_the_key() {
        let mut tree = parse(r#"{"a":1,"b":2}"#).unwrap();
        assert!(tree.remove("a"));
        assert!(!tree.remove("a"));
        assert_eq!(serde_json::to_string(&tree).unwrap(), r#"{"b":2}"#);
    }
}
