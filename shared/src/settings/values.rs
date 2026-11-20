//! The values of the spec's settings in a [`Config`], as `GET /settings` sends them.

use serde::{Deserialize, Serialize};
use serde_json::Value;

use super::{SettingControl, SettingsSpec};
use crate::{
    config::{Config, HeavyRule, Sizing},
    types::{JobClass, JobProfile},
};

/// The value of one setting on a page, keyed like the spec, in the kind its control edits.
///
/// The tags end in `_value`, so the panels' generated types never share a name with
/// [`SettingControl`]'s: System.Text.Json's source generator warns on a repeated type name.
#[cfg_attr(any(feature = "schema", test), derive(schemars::JsonSchema))]
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(tag = "kind")]
#[non_exhaustive]
pub enum SettingValue {
    /// The state of a switch.
    #[serde(rename = "switch_value")]
    Switch {
        /// The setting's key.
        key: String,
        /// Whether it is on.
        value: bool,
    },
    /// A number, in the unit of its setting.
    #[serde(rename = "number_value")]
    Number {
        /// The setting's key.
        key: String,
        /// The value.
        value: f64,
    },
    /// A list of strings, such as `classifier.light_flags`.
    #[serde(rename = "list_value")]
    List {
        /// The setting's key.
        key: String,
        /// The entries, in file order.
        value: Vec<String>,
    },
    /// The heavy-command rules.
    #[serde(rename = "rules_value")]
    Rules {
        /// The setting's key, `classifier.rules`.
        key: String,
        /// The rules, in matching order.
        value: Vec<HeavyRule>,
    },
    /// Every row of a size table, built-in rows included.
    #[serde(rename = "rows_value")]
    Rows {
        /// The table's key, `classes` or `presets`.
        key: String,
        /// The rows: classes in class order, presets by name.
        value: Vec<SizeRow>,
    },
}

/// A class or preset with its values.
#[cfg_attr(any(feature = "schema", test), derive(schemars::JsonSchema))]
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct SizeRow {
    /// The row's key in the file, such as `cpu_heavy` or a preset's name.
    pub name: String,
    /// What a lease of it asks for.
    pub profile: JobProfile,
    /// The fewest cores a lease of it may start with; `None` while it is automatic.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub min_cores: Option<f32>,
}

impl SizeRow {
    fn new(name: &str, sizing: &Sizing) -> Self {
        Self {
            name: name.to_owned(),
            profile: sizing.profile,
            min_cores: sizing.min_cores,
        }
    }
}

impl SettingValue {
    /// Every setting on `spec`'s pages with its value in `config`, in window order.
    pub fn all(spec: &SettingsSpec, config: &Config) -> Vec<Self> {
        let tree = scalar_tree(config);
        spec.settings()
            .filter_map(|setting| {
                let key = setting.key.clone();
                let found = lookup(&tree, &key);
                match &setting.control {
                    SettingControl::Switch { .. } => {
                        found?.as_bool().map(|value| Self::Switch { key, value })
                    }
                    SettingControl::Number { .. } => {
                        found?.as_f64().map(|value| Self::Number { key, value })
                    }
                    SettingControl::StringList { .. } => {
                        let value = found?
                            .as_array()?
                            .iter()
                            .map(|item| item.as_str().map(str::to_owned))
                            .collect::<Option<Vec<String>>>()?;
                        Some(Self::List { key, value })
                    }
                    SettingControl::RuleList { .. } => Some(Self::Rules {
                        key,
                        value: config.classifier.rules.clone(),
                    }),
                    SettingControl::SizeTable { .. } => {
                        size_rows(config, &key).map(|value| Self::Rows { key, value })
                    }
                    // Text and choices are only fields of rules, never settings on a page.
                    SettingControl::Text { .. } | SettingControl::Choice { .. } => None,
                }
            })
            .collect()
    }

    /// The setting's key.
    pub fn key(&self) -> &str {
        match self {
            Self::Switch { key, .. }
            | Self::Number { key, .. }
            | Self::List { key, .. }
            | Self::Rules { key, .. }
            | Self::Rows { key, .. } => key,
        }
    }
}

/// `config` as JSON, read back so each `f32` keeps its short spelling: `0.1`, not the
/// `0.10000000149011612` a direct conversion to `f64` gives.
fn scalar_tree(config: &Config) -> Value {
    serde_json::to_string(config)
        .ok()
        .and_then(|text| serde_json::from_str(&text).ok())
        .unwrap_or(Value::Null)
}

fn lookup<'a>(tree: &'a Value, key: &str) -> Option<&'a Value> {
    key.split('.').try_fold(tree, |node, part| node.get(part))
}

fn size_rows(config: &Config, key: &str) -> Option<Vec<SizeRow>> {
    match key {
        "classes" => Some(
            JobClass::ALL
                .iter()
                .map(|&class| SizeRow::new(class.as_str(), config.classes.get(class)))
                .collect(),
        ),
        "presets" => Some(
            config
                .presets
                .0
                .iter()
                .map(|(name, sizing)| SizeRow::new(name, sizing))
                .collect(),
        ),
        _ => None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn value_of<'a>(values: &'a [SettingValue], key: &str) -> &'a SettingValue {
        values
            .iter()
            .find(|value| value.key() == key)
            .unwrap_or_else(|| panic!("no value for {key}"))
    }

    #[test]
    fn test_all_gives_every_spec_setting_a_value_of_its_kind() {
        let spec = SettingsSpec::new();
        let values = SettingValue::all(&spec, &Config::default());
        let keys: Vec<&str> = values.iter().map(SettingValue::key).collect();
        let spec_keys: Vec<&str> = spec
            .settings()
            .map(|setting| setting.key.as_str())
            .collect();
        assert_eq!(keys, spec_keys);
        for (setting, value) in spec.settings().zip(&values) {
            let matches = matches!(
                (&setting.control, value),
                (SettingControl::Switch { .. }, SettingValue::Switch { .. })
                    | (SettingControl::Number { .. }, SettingValue::Number { .. })
                    | (SettingControl::StringList { .. }, SettingValue::List { .. })
                    | (SettingControl::RuleList { .. }, SettingValue::Rules { .. })
                    | (SettingControl::SizeTable { .. }, SettingValue::Rows { .. })
            );
            assert!(matches, "{} has a value of the wrong kind", setting.key);
        }
    }

    #[test]
    fn test_all_numbers_keep_their_short_spelling() {
        let mut config = Config::default();
        config.admission.headroom_pct = 12.3;
        let values = SettingValue::all(&SettingsSpec::new(), &config);
        assert_eq!(
            value_of(&values, "admission.headroom_pct"),
            &SettingValue::Number {
                key: "admission.headroom_pct".into(),
                value: 12.3
            }
        );
    }

    #[test]
    fn test_all_rows_hold_the_configured_sizes_and_minimums() {
        let mut config = Config::default();
        config.classes.mixed.min_cores = Some(2.5);
        let values = SettingValue::all(&SettingsSpec::new(), &config);
        let SettingValue::Rows { value: rows, .. } = value_of(&values, "classes") else {
            panic!("classes is not a table");
        };
        let names: Vec<&str> = rows.iter().map(|row| row.name.as_str()).collect();
        assert_eq!(names, ["light", "cpu_heavy", "gpu_heavy", "mixed"]);
        assert_eq!(rows[3].min_cores, Some(2.5));
        assert_eq!(rows[1].min_cores, None);
        let SettingValue::Rows { value: rows, .. } = value_of(&values, "presets") else {
            panic!("presets is not a table");
        };
        assert!(rows.iter().any(|row| row.name == "low_impact"));
    }
}
