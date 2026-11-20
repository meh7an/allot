//! Comment-preserving edits of single values, and the file's line endings and byte order mark.
//!
//! `toml_edit` keeps every byte it parsed, but two of its conveniences drop what the user
//! wrote: `Table::insert` over an existing key resets that key's formatting, which deletes
//! the comments above it, and a value built with `value(..)` has default decor, which drops
//! a trailing comment. These functions change values in place instead.

use toml_edit::{Decor, DocumentMut, Item, RawString, Table, TableLike, Value};

/// How a file was saved, beyond its TOML: `toml_edit` reads Windows line endings and a byte
/// order mark but prints `\n` everywhere and no mark, so an untouched CRLF file would come
/// back with every line changed.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) struct Layout {
    /// Starts with a UTF-8 byte order mark.
    bom: bool,
    /// Every line ends in `\r\n`. A file with mixed endings counts as `\n` and is written so.
    crlf: bool,
}

impl Layout {
    /// The layout of `text`.
    pub(super) fn of(text: &str) -> Self {
        let body = text.strip_prefix('\u{feff}');
        let lines = body.unwrap_or(text);
        Self {
            bom: body.is_some(),
            crlf: lines.contains("\r\n") && !lines.replace("\r\n", "").contains('\n'),
        }
    }

    /// `doc` as text in this layout.
    pub(super) fn render(self, doc: &DocumentMut) -> String {
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

/// A table an edit has to go through holds a plain value in the file, such as
/// `estimates = 3` under an edit of `estimates.min_runs`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(super) struct NotATable {
    /// The dotted path of that value.
    pub(super) path: String,
}

/// Sets the value at `path`, for example `["estimates", "min_runs"]`, creating missing
/// tables. An existing value keeps its spacing and trailing comment, and its key keeps the
/// comments above it. An empty `path` changes nothing.
///
/// # Errors
///
/// [`NotATable`] when `path` runs through a value that is not a table.
pub(super) fn set(doc: &mut DocumentMut, path: &[&str], value: Value) -> Result<(), NotATable> {
    let Some((key, parents)) = path.split_last() else {
        return Ok(());
    };
    let table = table_at(doc, parents)?;
    let mut value = value;
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
    Ok(())
}

/// Removes the key or table at `path` so its default applies again, with the comments
/// directly above it. Tables left empty are removed too, unless they carry a comment.
/// Returns whether anything was removed.
pub(super) fn remove(doc: &mut DocumentMut, path: &[&str]) -> bool {
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

/// Whether the file has a key or table at `path`.
pub(super) fn contains(doc: &DocumentMut, path: &[&str]) -> bool {
    let Some((key, parents)) = path.split_last() else {
        return false;
    };
    let mut table: &dyn TableLike = doc.as_table();
    for name in parents {
        match table.get(name).and_then(Item::as_table_like) {
            Some(child) => table = child,
            None => return false,
        }
    }
    table.contains_key(key)
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

/// The position after every table in the document, so a table given it prints last.
pub(super) fn next_position(doc: &DocumentMut) -> isize {
    highest_position(doc.as_table(), None).unwrap_or(0) + 1
}

/// The highest position of a table in `table`'s subtree, counting `own`, the position of
/// `table` itself; `None` when no table there has one.
fn highest_position(table: &dyn TableLike, own: Option<isize>) -> Option<isize> {
    table.iter().fold(own, |high, (_, item)| match item {
        Item::Table(child) => high.max(highest_position(child, child.position())),
        Item::ArrayOfTables(array) => array
            .iter()
            .map(|child| highest_position(child, child.position()))
            .fold(high, Option::max),
        _ => high,
    })
}

/// The table at `parents`, created where missing.
///
/// A new table directly under the root goes to the end of the file; left without a position,
/// `toml_edit` would print it after whichever table it visited last, which can be mid-file.
/// A new table under an existing one takes the highest position in that table's subtree, so
/// it prints right after the table's last block: tables print in position order, and a later
/// one keeps its place among equal positions.
fn table_at<'a>(
    doc: &'a mut DocumentMut,
    parents: &[&str],
) -> Result<&'a mut dyn TableLike, NotATable> {
    let end = next_position(doc);
    let mut table: &mut dyn TableLike = doc.as_table_mut();
    let mut own_position = None;
    let mut created_parent = false;
    for (depth, name) in parents.iter().enumerate() {
        let is_leaf = depth + 1 == parents.len();
        let position = if depth == 0 || created_parent {
            Some(end)
        } else {
            highest_position(&*table, own_position)
        };
        let mut created = false;
        let item = table.entry(name).or_insert_with(|| {
            created = true;
            let mut new = Table::new();
            new.set_implicit(!is_leaf);
            new.set_position(position);
            Item::Table(new)
        });
        created_parent = created;
        own_position = item.as_table().and_then(Table::position);
        table = item.as_table_like_mut().ok_or_else(|| NotATable {
            path: parents[..=depth].join("."),
        })?;
    }
    Ok(table)
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
        let Some((name, above)) = parents[..depth].split_last() else {
            return;
        };
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

fn has_comment(decor: &Decor) -> bool {
    let text = |raw: Option<&RawString>| {
        raw.and_then(|raw| raw.as_str())
            .is_some_and(|text| text.contains('#'))
    };
    text(decor.prefix()) || text(decor.suffix())
}
