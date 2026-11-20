//! Whole-list writes of `classifier.rules`.
//!
//! A rule that is still in the new list keeps its table as the user wrote it, comments
//! included; a new rule is written plainly; a dropped rule goes with its comments. The list
//! keeps the form the user chose: `[[classifier.rules]]` tables or an inline array.

use toml_edit::{Array, ArrayOfTables, DocumentMut, InlineTable, Item, Table, TableLike, Value};

use super::edit::{self, NotATable};
use crate::{config::HeavyRule, types::JobClass};

const CLASSIFIER: &str = "classifier";

/// Writes `rules` as the whole `classifier.rules` list. An empty list is written as
/// `rules = []`, since leaving the key out would bring back the built-in rules.
///
/// # Errors
///
/// [`NotATable`] when `classifier` is not a table, or `classifier.rules` is neither tables
/// nor an array.
pub(super) fn set(doc: &mut DocumentMut, rules: &[HeavyRule]) -> Result<(), NotATable> {
    if rules.is_empty() {
        return set_empty(doc);
    }
    let end = edit::next_position(doc);
    let classifier = classifier(doc, end)?;
    let classifier_position = classifier.position();
    match classifier.get_mut("rules") {
        Some(Item::ArrayOfTables(tables)) => rewrite_tables(tables, rules, end),
        Some(Item::Value(Value::Array(array))) => rewrite_inline(array, rules),
        Some(_) => return Err(rules_not_a_list()),
        None => {
            let mut tables = ArrayOfTables::new();
            rewrite_tables(&mut tables, rules, classifier_position.unwrap_or(end));
            classifier.insert("rules", Item::ArrayOfTables(tables));
        }
    }
    Ok(())
}

/// Removes `classifier.rules`, so the built-in rules apply again.
pub(super) fn reset(doc: &mut DocumentMut) -> bool {
    edit::remove(doc, &[CLASSIFIER, "rules"])
}

fn rules_not_a_list() -> NotATable {
    NotATable {
        path: format!("{CLASSIFIER}.rules"),
    }
}

/// The `[classifier]` table, created at `end` when missing.
fn classifier(doc: &mut DocumentMut, end: isize) -> Result<&mut Table, NotATable> {
    doc.as_table_mut()
        .entry(CLASSIFIER)
        .or_insert_with(|| {
            let mut table = Table::new();
            table.set_implicit(true);
            table.set_position(Some(end));
            Item::Table(table)
        })
        .as_table_mut()
        .ok_or_else(|| NotATable {
            path: CLASSIFIER.to_owned(),
        })
}

fn set_empty(doc: &mut DocumentMut) -> Result<(), NotATable> {
    let end = edit::next_position(doc);
    let classifier = classifier(doc, end)?;
    let first_rule = classifier
        .get("rules")
        .and_then(Item::as_array_of_tables)
        .and_then(|tables| tables.iter().find_map(Table::position));
    if classifier.is_implicit() {
        // An implicit table has no header, and `rules = []` needs one to sit under.
        classifier.set_implicit(false);
        classifier.set_position(first_rule.or(Some(end)));
    }
    match classifier.get_mut("rules") {
        Some(Item::Value(Value::Array(array))) => array.clear(),
        Some(Item::Value(_)) => return Err(rules_not_a_list()),
        _ => {
            classifier.remove("rules");
            classifier.insert("rules", Item::Value(Value::Array(Array::new())));
        }
    }
    Ok(())
}

/// Rewrites `[[classifier.rules]]`. Every table gets one position, the first old table's (or
/// `fallback`), so they print together in list order: the printer sorts tables by position
/// and keeps the list order among equal ones.
fn rewrite_tables(tables: &mut ArrayOfTables, rules: &[HeavyRule], fallback: isize) {
    let position = tables.iter().find_map(Table::position).unwrap_or(fallback);
    let mut old: Vec<Option<Table>> = tables.iter().cloned().map(Some).collect();
    tables.clear();
    for rule in rules {
        let mut table = take_matching(&mut old, rule, |table| parse_rule(table))
            .unwrap_or_else(|| rule_table(rule));
        table.set_position(Some(position));
        tables.push(table);
    }
}

/// One entry of an inline list with the text around it split by owner.
///
/// `toml_edit` gives everything between a comma and the next entry to the next entry, so in
/// `{ .. },  # note` the note belongs to the entry below. Users mean the entry on the same
/// line, so the text up to the first line break goes with the entry before it.
#[derive(Debug)]
struct Entry {
    value: Value,
    /// From the first line break up to the entry: comment lines above it and its indent.
    lead: String,
    /// The rest of the entry's line after its comma, such as `  # make -j everything`.
    tail: String,
}

/// Splits `raw` at its first line break: what stays on the line above, and the rest.
fn split_line(raw: &str) -> (&str, &str) {
    match raw.find('\n') {
        Some(at) => raw.split_at(at),
        None => ("", raw),
    }
}

fn raw_prefix(value: &Value) -> &str {
    value
        .decor()
        .prefix()
        .and_then(|raw| raw.as_str())
        .unwrap_or("")
}

/// Rewrites `rules = [ ... ]`. Kept entries take their comments along, and new entries take
/// the indent of the first old one, so a list written one entry per line stays that way.
fn rewrite_inline(array: &mut Array, rules: &[HeavyRule]) {
    let trailing = array.trailing().as_str().unwrap_or("").to_owned();
    let values: Vec<Value> = array.iter().cloned().collect();
    let opening = values
        .first()
        .map_or("", |first| split_line(raw_prefix(first)).0)
        .to_owned();
    let indent = values.first().and_then(|first| {
        let lead = split_line(raw_prefix(first)).1;
        lead.rfind('\n').map(|at| lead[at..].to_owned())
    });
    let mut old: Vec<Option<Entry>> = values
        .iter()
        .enumerate()
        .map(|(index, value)| {
            let next = values.get(index + 1).map_or(trailing.as_str(), raw_prefix);
            Some(Entry {
                value: value.clone(),
                lead: split_line(raw_prefix(value)).1.to_owned(),
                tail: split_line(next).0.to_owned(),
            })
        })
        .collect();

    array.clear();
    let mut above = opening;
    for rule in rules {
        let entry = take_matching(&mut old, rule, |entry| {
            entry
                .value
                .as_inline_table()
                .and_then(|table| parse_rule(table))
        })
        .unwrap_or_else(|| Entry {
            value: Value::InlineTable(rule_inline(rule)),
            lead: indent.clone().unwrap_or_else(|| " ".to_owned()),
            tail: String::new(),
        });
        let mut value = entry.value;
        value
            .decor_mut()
            .set_prefix(format!("{above}{}", entry.lead));
        array.push_formatted(value);
        above = entry.tail;
    }
    array.set_trailing(format!("{above}{}", split_line(&trailing).1));
}

/// Takes the first unused old entry that reads as `rule`.
fn take_matching<T>(
    old: &mut [Option<T>],
    rule: &HeavyRule,
    read: impl Fn(&T) -> Option<HeavyRule>,
) -> Option<T> {
    let index = old
        .iter()
        .position(|entry| entry.as_ref().and_then(&read).as_ref() == Some(rule))?;
    old[index].take()
}

fn parse_rule(table: &dyn TableLike) -> Option<HeavyRule> {
    let program = table.get("program")?.as_str()?.to_owned();
    let args = match table.get("args") {
        Some(args) => args
            .as_array()?
            .iter()
            .map(|arg| arg.as_str().map(str::to_owned))
            .collect::<Option<Vec<_>>>()?,
        None => Vec::new(),
    };
    let class_name = table.get("class")?.as_str()?;
    let class = JobClass::ALL
        .into_iter()
        .find(|class| class.as_str() == class_name)?;
    Some(HeavyRule {
        program,
        args,
        class,
    })
}

fn rule_table(rule: &HeavyRule) -> Table {
    let mut table = Table::new();
    for (key, value) in rule_values(rule) {
        table.insert(key, Item::Value(value));
    }
    table
}

fn rule_inline(rule: &HeavyRule) -> InlineTable {
    rule_values(rule).into_iter().collect()
}

/// A rule's keys in the order users write them; `args` is left out when empty.
fn rule_values(rule: &HeavyRule) -> Vec<(&'static str, Value)> {
    let mut values = vec![("program", Value::from(rule.program.as_str()))];
    if !rule.args.is_empty() {
        let args: Array = rule.args.iter().map(String::as_str).collect();
        values.push(("args", Value::Array(args)));
    }
    values.push(("class", Value::from(rule.class.as_str())));
    values
}
