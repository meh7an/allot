//! Comment-preserving edits of single values and whole tables.
//!
//! `toml_edit` keeps every byte it parsed, but two of its conveniences drop what the user
//! wrote: `Table::insert` over an existing key resets that key's formatting, which deletes
//! the comments above it, and a value built with `value(..)` has default decor, which drops
//! a trailing comment. These functions change values in place instead.

use toml_edit::{DocumentMut, Item, Table, TableLike, Value};

/// How a file was saved, beyond its TOML: `toml_edit` reads Windows line endings and a byte
/// order mark but prints `\n` everywhere and no mark, so an untouched CRLF file would come
/// back with every line changed.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Layout {
    /// Starts with a UTF-8 byte order mark.
    pub bom: bool,
    /// Every line ends in `\r\n`. A file with mixed endings counts as `\n` and is written so.
    pub crlf: bool,
}

impl Layout {
    /// The layout of `text`.
    pub fn of(text: &str) -> Self {
        let body = text.strip_prefix('\u{feff}');
        let lines = body.unwrap_or(text);
        Self {
            bom: body.is_some(),
            crlf: lines.contains("\r\n") && !lines.replace("\r\n", "").contains('\n'),
        }
    }

    /// `doc` as text in this layout.
    pub fn render(self, doc: &DocumentMut) -> String {
        let text = doc.to_string();
        let text = if self.crlf {
            text.replace('\n', "\r\n")
        } else {
            text
        };
        if self.bom {
            format!("\u{feff}{text}")
        } else {
            text
        }
    }
}

/// Sets the value at `path`, for example `["estimates", "min_runs"]`, creating missing
/// tables. An existing value keeps its spacing and trailing comment, and its key keeps the
/// comments above it.
///
/// # Panics
///
/// Panics when `path` is empty or runs through a value that is not a table.
pub fn set(doc: &mut DocumentMut, path: &[&str], value: impl Into<Value>) {
    let (key, parents) = path.split_last().expect("a path names at least one key");
    let table = table_at(doc, parents);
    let mut value = value.into();
    match table.get_mut(key) {
        Some(Item::Value(old)) if same_value(old, &value) => {}
        Some(Item::Value(old)) => {
            *value.decor_mut() = old.decor().clone();
            *old = value;
        }
        Some(item) => *item = Item::Value(value),
        None => {
            table.insert(key, Item::Value(value));
        }
    }
}

/// Removes the key or table at `path` so its default applies again, with the comments
/// directly above it. Tables left empty are removed too, unless they carry a comment.
/// Returns whether anything was removed.
pub fn remove(doc: &mut DocumentMut, path: &[&str]) -> bool {
    let Some((key, parents)) = path.split_last() else {
        return false;
    };
    let Some(table) = existing_table(doc.as_table_mut(), parents) else {
        return false;
    };
    if table.remove(key).is_none() {
        return false;
    }
    prune_empty(doc.as_table_mut(), parents);
    true
}

/// Whether `new` means what `old` already says, so the user's spelling (`6` rather than
/// `6.0`, a literal string rather than a basic one) stays. Lists and tables always count as
/// changed.
fn same_value(old: &Value, new: &Value) -> bool {
    let number = |value: &Value| {
        value
            .as_float()
            .or_else(|| value.as_integer().map(|int| int as f64))
    };
    match (old, new) {
        (Value::String(old), Value::String(new)) => old.value() == new.value(),
        (Value::Boolean(old), Value::Boolean(new)) => old.value() == new.value(),
        _ => number(old).is_some_and(|old| Some(old) == number(new)),
    }
}

/// Makes the table at `path` hold exactly `entries`: existing keys change in place, missing
/// ones are appended in the given order, and keys not listed are removed.
pub fn set_table(doc: &mut DocumentMut, path: &[&str], entries: &[(&str, Value)]) {
    let mut full = path.to_vec();
    for (key, value) in entries {
        full.push(key);
        set(doc, &full, value.clone());
        full.pop();
    }
    let table = table_at(doc, path);
    let stale: Vec<String> = table
        .iter()
        .map(|(key, _)| key.to_owned())
        .filter(|key| entries.iter().all(|(wanted, _)| wanted != key))
        .collect();
    for key in stale {
        table.remove(&key);
    }
}

/// The position after every table in the document, so a table given it prints last.
pub fn next_position(doc: &DocumentMut) -> isize {
    fn highest(table: &Table) -> isize {
        let own = table.position().unwrap_or(0);
        table.iter().fold(own, |high, (_, item)| match item {
            Item::Table(child) => high.max(highest(child)),
            Item::ArrayOfTables(array) => array.iter().map(highest).fold(high, isize::max),
            _ => high,
        })
    }
    highest(doc.as_table()) + 1
}

/// The table at `parents`, created where missing.
///
/// A new table directly under the root goes to the end of the file; left without a position,
/// `toml_edit` would print it after whichever table it visited last, which can be mid-file.
/// A new table under an existing one keeps no position, so it prints after its siblings.
fn table_at<'a>(doc: &'a mut DocumentMut, parents: &[&str]) -> &'a mut dyn TableLike {
    let end = next_position(doc);
    let mut table: &mut dyn TableLike = doc.as_table_mut();
    let mut created_parent = false;
    for (depth, name) in parents.iter().enumerate() {
        let is_leaf = depth + 1 == parents.len();
        let at_root = depth == 0;
        let mut created = false;
        let item = table.entry(name).or_insert_with(|| {
            created = true;
            let mut new = Table::new();
            new.set_implicit(!is_leaf);
            if at_root || created_parent {
                new.set_position(Some(end));
            }
            Item::Table(new)
        });
        created_parent = created;
        table = item
            .as_table_like_mut()
            .unwrap_or_else(|| panic!("`{name}` is a value, not a table"));
    }
    table
}

fn existing_table<'a>(root: &'a mut Table, parents: &[&str]) -> Option<&'a mut dyn TableLike> {
    let mut table: &mut dyn TableLike = root;
    for name in parents {
        table = table.get_mut(name)?.as_table_like_mut()?;
    }
    Some(table)
}

/// Removes the tables along `parents` that are now empty and hold no comment, deepest first.
fn prune_empty(root: &mut Table, parents: &[&str]) {
    for depth in (1..=parents.len()).rev() {
        let (name, above) = parents[..depth].split_last().expect("depth is at least 1");
        let Some(parent) = existing_table(root, above) else {
            return;
        };
        let Some((key, item)) = parent.get_key_value(name) else {
            return;
        };
        let empty = item.as_table_like().is_some_and(|table| table.is_empty());
        let commented = has_comment(key.leaf_decor())
            || item
                .as_table()
                .is_some_and(|table| has_comment(table.decor()));
        if !empty || commented {
            return;
        }
        parent.remove(name);
    }
}

fn has_comment(decor: &toml_edit::Decor) -> bool {
    let text = |raw: Option<&toml_edit::RawString>| {
        raw.and_then(|raw| raw.as_str())
            .is_some_and(|text| text.contains('#'))
    };
    text(decor.prefix()) || text(decor.suffix())
}
