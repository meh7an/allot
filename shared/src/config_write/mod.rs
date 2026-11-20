//! Changes to `config.toml` that keep everything the user wrote.
//!
//! allotd is the only writer of the file. A batch of [`Change`]s is checked against the
//! [`SettingsSpec`], applied in place with comments, blank lines, key order, line endings and
//! byte order mark kept, validated as a whole with [`Config::problems`], and written
//! atomically, but only while the file still has the revision the caller read. Nothing is
//! written unless every change is valid.

mod edit;
mod file;
mod rules;

use std::{io, path::Path};

use serde::{Deserialize, Serialize};
use toml_edit::{DocumentMut, Value};

pub use self::file::{ParseRevisionError, Revision};
use self::{
    edit::{Layout, NotATable},
    file::{HELD_RETRY, ReplaceError},
};
use crate::{
    config::{Config, FieldError, HeavyRule},
    settings::{SettingControl, SettingSpec, SettingsSpec, SizeRowSpec},
    types::JobProfile,
};

/// One change to `config.toml`. Keys are the spec's dotted keys, such as
/// `admission.headroom_pct`; tables are a size table's key, `classes` or `presets`.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
#[non_exhaustive]
pub enum Change {
    /// Turns a switch on or off.
    SetSwitch {
        /// The switch's key.
        key: String,
        /// The new state.
        value: bool,
    },
    /// Sets a number, in the unit of its setting. A whole number is written as an integer
    /// for whole-number settings and as a float, such as `8.0`, for the others.
    SetNumber {
        /// The number's key.
        key: String,
        /// The new value.
        value: f64,
    },
    /// Replaces a list of strings, such as `classifier.light_flags`, as a whole.
    SetList {
        /// The list's key.
        key: String,
        /// The new list.
        value: Vec<String>,
    },
    /// Replaces `classifier.rules` as a whole. An empty list is written as `rules = []`, so
    /// no rule applies; [`Change::Reset`] of `classifier.rules` brings back the built-in ones.
    SetRules {
        /// The rules, in matching order.
        rules: Vec<HeavyRule>,
    },
    /// Sets every field of a class or preset, adding the preset when it is new. A field is
    /// written when the file already has it or it differs from the row's default.
    SetRow {
        /// The size table, `classes` or `presets`.
        table: String,
        /// The row, such as `cpu_heavy` or a preset's name.
        name: String,
        /// The row's values.
        profile: JobProfile,
        /// The fewest cores a lease of it may start with; `None` leaves it automatic.
        #[serde(default, skip_serializing_if = "Option::is_none")]
        min_cores: Option<f32>,
    },
    /// Removes a class or preset from the file: a built-in row takes its defaults again, and
    /// a preset of the user's own is deleted. A rename is this and a [`Change::SetRow`].
    ResetRow {
        /// The size table, `classes` or `presets`.
        table: String,
        /// The row.
        name: String,
    },
    /// Removes a setting from the file so its default applies, or with a size table's key,
    /// every row of it.
    Reset {
        /// The setting's key.
        key: String,
    },
}

/// The file as read: its text and revision.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Snapshot {
    /// The file's text; empty when the file does not exist.
    pub text: String,
    /// The revision to pass to [`apply`].
    pub revision: Revision,
}

/// A batch of changes applied in memory.
#[derive(Debug, Clone, PartialEq)]
pub struct Edited {
    /// The new text of the file.
    pub text: String,
    /// The configuration the new text holds.
    pub config: Config,
}

/// What a successful [`apply`] did.
#[derive(Debug, Clone, PartialEq)]
pub struct Applied {
    /// The file's revision now.
    pub revision: Revision,
    /// The configuration the file now holds.
    pub config: Config,
    /// Whether the file was replaced; `false` when the changes left its bytes as they were.
    pub written: bool,
}

/// Why a batch of changes was not written. The file is untouched in every case.
#[derive(Debug, thiserror::Error)]
#[non_exhaustive]
pub enum WriteError {
    /// Values the result would not allow, each with its key and what it must be, including
    /// problems the file already had that the changes did not fix.
    #[error("{}", join(.0))]
    Invalid(Vec<FieldError>),
    /// The file is not something the changes can be applied to: not TOML, not UTF-8, or a
    /// key the configuration does not know.
    #[error("config.toml cannot be changed: {0}")]
    File(String),
    /// The file changed since it was read, most likely by hand. Read it again and reapply.
    #[error("config.toml changed since it was read")]
    Conflict {
        /// The revision on disk now.
        current: Revision,
    },
    /// Another program kept the file open in a way that blocks changes, for about a second.
    #[error(
        "config.toml is open in another program that blocks changes. Close it there and try \
         again."
    )]
    Held(#[source] io::Error),
    /// Reading or writing the file failed.
    #[error(transparent)]
    Io(io::Error),
}

impl From<ReplaceError> for WriteError {
    fn from(error: ReplaceError) -> Self {
        match error {
            ReplaceError::Conflict { current } => Self::Conflict { current },
            ReplaceError::Held(error) => Self::Held(error),
            ReplaceError::Io(error) => Self::Io(error),
        }
    }
}

fn join(problems: &[FieldError]) -> String {
    let text: Vec<String> = problems.iter().map(ToString::to_string).collect();
    text.join("; ")
}

/// Reads the file at `path`, following symbolic links; a missing file reads as empty.
///
/// # Errors
///
/// [`WriteError::Held`] when another program keeps the file from being read,
/// [`WriteError::File`] when it is not UTF-8, and [`WriteError::Io`] for other failures.
pub fn read(path: &Path) -> Result<Snapshot, WriteError> {
    let target = file::resolve(path).map_err(WriteError::Io)?;
    let bytes = file::read_bytes(&target).map_err(ReplaceError::from)?;
    let revision = Revision::of(&bytes);
    let text = String::from_utf8(bytes)
        .map_err(|_| WriteError::File("the file is not UTF-8 text".to_owned()))?;
    Ok(Snapshot { text, revision })
}

/// Applies `changes` to the file at `path` if it still has revision `expected`, and returns
/// the new revision and configuration. A symbolic link stays a link to its replaced target.
///
/// # Errors
///
/// [`WriteError::Conflict`] when the file changed since `expected` was read,
/// [`WriteError::Invalid`] naming every key the result would get wrong, and the errors of
/// [`read`] and [`edit`]. The file is left as it was.
pub fn apply(path: &Path, expected: Revision, changes: &[Change]) -> Result<Applied, WriteError> {
    let target = file::resolve(path).map_err(WriteError::Io)?;
    let snapshot = read(&target)?;
    if snapshot.revision != expected {
        return Err(WriteError::Conflict {
            current: snapshot.revision,
        });
    }
    let edited = edit(&snapshot.text, changes)?;
    if edited.text == snapshot.text {
        return Ok(Applied {
            revision: snapshot.revision,
            config: edited.config,
            written: false,
        });
    }
    file::replace(&target, &edited.text, expected, HELD_RETRY)?;
    Ok(Applied {
        revision: Revision::of(edited.text.as_bytes()),
        config: edited.config,
        written: true,
    })
}

/// Applies `changes` to `text`, the contents of a `config.toml`, without touching any file.
///
/// # Errors
///
/// [`WriteError::Invalid`] when a change names no setting, has the wrong kind of value, or
/// leaves a value out of range; [`WriteError::File`] when `text` is not TOML or holds a key
/// the configuration does not know.
pub fn edit(text: &str, changes: &[Change]) -> Result<Edited, WriteError> {
    let mut doc: DocumentMut = text
        .parse()
        .map_err(|error| WriteError::File(format!("not valid TOML: {error}")))?;
    let spec = SettingsSpec::new();
    let problems: Vec<FieldError> = changes
        .iter()
        .filter_map(|change| apply_change(&mut doc, &spec, change).err())
        .collect();
    if !problems.is_empty() {
        return Err(WriteError::Invalid(problems));
    }
    let text = Layout::of(text).render(&doc);
    let config: Config =
        toml::from_str(&text).map_err(|error| WriteError::File(error.to_string()))?;
    let problems = config.problems();
    if !problems.is_empty() {
        return Err(WriteError::Invalid(problems));
    }
    Ok(Edited { text, config })
}

fn apply_change(
    doc: &mut DocumentMut,
    spec: &SettingsSpec,
    change: &Change,
) -> Result<(), FieldError> {
    match change {
        Change::SetSwitch { key, value } => {
            match control(spec, key)? {
                SettingControl::Switch { .. } => {}
                other => return Err(wrong_kind(key, other)),
            }
            set(doc, key, Value::from(*value))
        }
        Change::SetNumber { key, value } => {
            let integer = match control(spec, key)? {
                SettingControl::Number { integer, .. } => *integer,
                other => return Err(wrong_kind(key, other)),
            };
            set(doc, key, number_value(key, *value, integer)?)
        }
        Change::SetList { key, value } => {
            match control(spec, key)? {
                SettingControl::StringList { .. } => {}
                other => return Err(wrong_kind(key, other)),
            }
            let list: toml_edit::Array = value.iter().map(String::as_str).collect();
            set(doc, key, Value::Array(list))
        }
        Change::SetRules { rules } => {
            rules::set(doc, rules).map_err(|blocked| not_a_table(RULES_KEY, &blocked))
        }
        Change::SetRow {
            table,
            name,
            profile,
            min_cores,
        } => set_row(doc, spec, table, name, profile, *min_cores),
        Change::ResetRow { table, name } => {
            size_table(spec, table)?;
            edit::remove(doc, &[table, name]);
            Ok(())
        }
        Change::Reset { key } => {
            control(spec, key)?;
            if key == RULES_KEY {
                rules::reset(doc);
            } else {
                edit::remove(doc, &path(key));
            }
            Ok(())
        }
    }
}

const RULES_KEY: &str = "classifier.rules";

fn path(key: &str) -> Vec<&str> {
    key.split('.').collect()
}

fn set(doc: &mut DocumentMut, key: &str, value: Value) -> Result<(), FieldError> {
    edit::set(doc, &path(key), value).map_err(|blocked| not_a_table(key, &blocked))
}

fn control<'a>(spec: &'a SettingsSpec, key: &str) -> Result<&'a SettingControl, FieldError> {
    spec.get(key)
        .map(|setting| &setting.control)
        .ok_or_else(|| FieldError::new(key, "is not a setting"))
}

/// The error for a change of the wrong kind for its setting, saying what the setting takes.
fn wrong_kind(key: &str, control: &SettingControl) -> FieldError {
    let takes = match control {
        SettingControl::Switch { .. } => "takes on or off",
        SettingControl::Number { .. } => "takes a number",
        SettingControl::StringList { .. } => "takes a list of text",
        SettingControl::SizeTable { .. } => "is a table, changed one row at a time",
        SettingControl::RuleList { .. } => "is the rule list, written whole",
        _ => "cannot be changed this way",
    };
    FieldError::new(key, takes)
}

fn not_a_table(key: &str, blocked: &NotATable) -> FieldError {
    FieldError::new(
        key,
        format!(
            "cannot be written while `{}` in the file is a value, not a table",
            blocked.path
        ),
    )
}

/// `value` as the TOML value a setting takes: an integer for whole-number settings, a float
/// otherwise.
fn number_value(key: &str, value: f64, integer: bool) -> Result<Value, FieldError> {
    if !integer {
        return Ok(Value::from(value));
    }
    if !(value.is_finite() && value.fract() == 0.0) {
        return Err(FieldError::new(key, "must be a whole number"));
    }
    if value < 0.0 {
        return Err(FieldError::new(key, "must be at least 0"));
    }
    // TOML integers are i64; a whole number at most i64::MAX converts exactly enough here, as
    // the parse below rejects anything the setting's own type cannot hold.
    if value > i64::MAX as f64 {
        return Err(FieldError::new(key, "is too large"));
    }
    let value = Value::from(value as i64);
    fits_type(key, &value)?;
    Ok(value)
}

/// Whether `value` fits the Rust type behind `key`, such as `u16` for `daemon.port`, read the
/// way allotd reads the file. Ranges are [`Config::problems`]'s concern; this only catches
/// what would make the whole file unreadable.
fn fits_type(key: &str, value: &Value) -> Result<(), FieldError> {
    let Some((table, leaf)) = key.rsplit_once('.') else {
        return Ok(());
    };
    let snippet = format!("[{table}]\n{leaf} = {}\n", value.to_string().trim());
    toml::from_str::<Config>(&snippet)
        .map(drop)
        .map_err(|_| FieldError::new(key, "is too large"))
}

/// A size table's built-in rows and fields, and whether rows may be added.
struct SizeTable<'a> {
    rows: &'a [SizeRowSpec],
    fields: &'a [SettingSpec],
    open: bool,
}

fn size_table<'a>(spec: &'a SettingsSpec, table: &str) -> Result<SizeTable<'a>, FieldError> {
    match control(spec, table)? {
        SettingControl::SizeTable {
            rows,
            fields,
            new_row_name,
        } => Ok(SizeTable {
            rows,
            fields,
            open: new_row_name.is_some(),
        }),
        other => Err(wrong_kind(table, other)),
    }
}

/// One field of a row write: its key, its value, and whether that value is the default the
/// row has while the file leaves the field out.
struct RowField {
    key: &'static str,
    value: Value,
    is_default: bool,
}

fn set_row(
    doc: &mut DocumentMut,
    spec: &SettingsSpec,
    table: &str,
    name: &str,
    profile: &JobProfile,
    min_cores: Option<f32>,
) -> Result<(), FieldError> {
    let sizes = size_table(spec, table)?;
    let row_key = format!("{table}.{name}");
    let builtin = sizes.rows.iter().find(|row| row.name == name);
    if builtin.is_none() && !sizes.open {
        return Err(FieldError::new(row_key, "is not a row of this table"));
    }
    // A new preset's defaults are the spec's: a field without one, like `cores`, is always
    // written, since a preset of the user's own must name it.
    let new_row_default = |field: &str| {
        sizes
            .fields
            .iter()
            .find(|setting| setting.key == field)
            .and_then(|setting| match &setting.control {
                SettingControl::Number { default, .. } => *default,
                _ => None,
            })
    };
    let is_default = |field: &str, value: f64, builtin_value: Option<f64>| match builtin {
        Some(_) => builtin_value == Some(value),
        None => new_row_default(field) == Some(value),
    };
    let base = builtin.map(|row| row.default);
    let mut fields = vec![
        RowField {
            key: "cores",
            value: float_value(profile.cores),
            is_default: is_default(
                "cores",
                f64::from(profile.cores),
                base.map(|base| f64::from(base.cores)),
            ),
        },
        integer_field(&row_key, "ram_mb", profile.ram_mb, |field, value| {
            is_default(field, value, base.map(|base| base.ram_mb as f64))
        })?,
        RowField {
            key: "gpu_pct",
            value: float_value(profile.gpu_pct),
            is_default: is_default(
                "gpu_pct",
                f64::from(profile.gpu_pct),
                base.map(|base| f64::from(base.gpu_pct)),
            ),
        },
        integer_field(&row_key, "vram_mb", profile.vram_mb, |field, value| {
            is_default(field, value, base.map(|base| base.vram_mb as f64))
        })?,
        integer_field(&row_key, "est_secs", profile.est_secs, |field, value| {
            is_default(field, value, base.map(|base| base.est_secs as f64))
        })?,
    ];
    if let Some(min_cores) = min_cores {
        let builtin_min = builtin.and_then(|row| row.default_min_cores);
        fields.push(RowField {
            key: "min_cores",
            value: float_value(min_cores),
            is_default: builtin.is_some() && builtin_min == Some(min_cores),
        });
    }
    for field in fields {
        let field_path = [table, name, field.key];
        if field.is_default && !edit::contains(doc, &field_path) {
            continue;
        }
        edit::set(doc, &field_path, field.value)
            .map_err(|blocked| not_a_table(&row_key, &blocked))?;
    }
    if min_cores.is_none() {
        // Last, so a row left empty is pruned rather than recreated at the end of the file.
        edit::remove(doc, &[table, name, "min_cores"]);
    }
    Ok(())
}

fn integer_field(
    row_key: &str,
    key: &'static str,
    value: u64,
    is_default: impl Fn(&str, f64) -> bool,
) -> Result<RowField, FieldError> {
    let integer = i64::try_from(value)
        .map_err(|_| FieldError::new(format!("{row_key}.{key}"), "is too large"))?;
    Ok(RowField {
        key,
        value: Value::from(integer),
        is_default: is_default(key, value as f64),
    })
}

/// `value` as a TOML float spelled the way `value` prints, so `0.1_f32` is written `0.1` and
/// not its exact binary value `0.10000000149011612`.
fn float_value(value: f32) -> Value {
    Value::from(value.to_string().parse::<f64>().unwrap_or(f64::from(value)))
}
