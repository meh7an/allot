//! IOKit registry entries and their properties as plain data. The live reader converts
//! CoreFoundation values into them; fixtures load the same shape from `ioreg -a` output
//! saved as JSON.

/// A property value.
#[derive(Debug, Clone, PartialEq)]
pub(crate) enum Value {
    Int(i64),
    Float(f64),
    Bool(bool),
    Str(String),
    Array(Vec<Value>),
    Dict(Vec<(String, Value)>),
    /// A type the sampler never reads, such as data or a date.
    Other,
}

impl Value {
    /// The value under `key` of a dictionary.
    pub(crate) fn get(&self, key: &str) -> Option<&Value> {
        match self {
            Self::Dict(entries) => entries
                .iter()
                .find(|(name, _)| name == key)
                .map(|(_, value)| value),
            _ => None,
        }
    }

    /// A non-negative number. Counters are integers, but CF numbers can arrive as floats.
    pub(crate) fn as_u64(&self) -> Option<u64> {
        match *self {
            Self::Int(number) => u64::try_from(number).ok(),
            Self::Float(number) if number >= 0.0 => Some(number as u64),
            _ => None,
        }
    }

    pub(crate) fn as_str(&self) -> Option<&str> {
        match self {
            Self::Str(text) => Some(text),
            _ => None,
        }
    }

    /// The items of an array; empty for anything else.
    pub(crate) fn items(&self) -> &[Value] {
        match self {
            Self::Array(items) => items,
            _ => &[],
        }
    }

    /// The non-negative number under `key`.
    pub(crate) fn u64_at(&self, key: &str) -> Option<u64> {
        self.get(key).and_then(Self::as_u64)
    }
}

/// One registry entry: its ID (unique until the next boot), the properties that were read,
/// and the children that were read.
#[derive(Debug, Clone, PartialEq)]
pub(crate) struct Entry {
    pub(crate) id: u64,
    pub(crate) properties: Value,
    pub(crate) children: Vec<Entry>,
}

#[cfg(test)]
impl Value {
    pub(crate) fn from_json(json: &serde_json::Value) -> Self {
        use serde_json::Value as Json;
        match json {
            Json::Bool(flag) => Self::Bool(*flag),
            Json::Number(number) => number
                .as_i64()
                .map(Self::Int)
                .or_else(|| number.as_f64().map(Self::Float))
                .unwrap_or(Self::Other),
            Json::String(text) => Self::Str(text.clone()),
            Json::Array(items) => Self::Array(items.iter().map(Self::from_json).collect()),
            Json::Object(entries) => Self::Dict(
                entries
                    .iter()
                    .map(|(key, value)| (key.clone(), Self::from_json(value)))
                    .collect(),
            ),
            Json::Null => Self::Other,
        }
    }
}

#[cfg(test)]
impl Entry {
    /// An entry as `ioreg -a` writes it: the properties next to `IORegistryEntryID` and
    /// `IORegistryEntryChildren`.
    pub(crate) fn from_ioreg(json: &serde_json::Value) -> Self {
        let children = json["IORegistryEntryChildren"]
            .as_array()
            .map(|children| children.iter().map(Self::from_ioreg).collect())
            .unwrap_or_default();
        Self {
            id: json["IORegistryEntryID"].as_u64().unwrap_or_default(),
            properties: Value::from_json(json),
            children,
        }
    }
}

#[cfg(test)]
mod tests {
    use serde_json::json;

    use super::*;

    #[test]
    fn test_value_reads_numbers_strings_and_nested_dictionaries() {
        let value = Value::from_json(&json!({
            "count": 3,
            "ratio": 2.5,
            "negative": -1,
            "name": "disk0",
            "stats": {"Bytes (Read)": 4096},
            "list": [1, 2],
        }));
        assert_eq!(value.u64_at("count"), Some(3));
        assert_eq!(value.u64_at("ratio"), Some(2));
        assert_eq!(value.u64_at("negative"), None);
        assert_eq!(value.get("name").and_then(Value::as_str), Some("disk0"));
        assert_eq!(
            value.get("stats").unwrap().u64_at("Bytes (Read)"),
            Some(4096)
        );
        assert_eq!(value.get("list").unwrap().items().len(), 2);
        assert!(value.get("missing").is_none());
        assert!(value.get("name").unwrap().items().is_empty());
    }

    #[test]
    fn test_entry_from_ioreg_reads_id_and_children() {
        let entry = Entry::from_ioreg(&json!({
            "IORegistryEntryID": 4_294_968_386_u64,
            "model": "Apple M3 Max",
            "IORegistryEntryChildren": [{"IORegistryEntryID": 7, "IOUserClientCreator": "pid 1"}],
        }));
        assert_eq!(entry.id, 4_294_968_386);
        assert_eq!(entry.children.len(), 1);
        assert_eq!(entry.children[0].id, 7);
        assert_eq!(
            entry.properties.get("model").and_then(Value::as_str),
            Some("Apple M3 Max")
        );
    }
}
