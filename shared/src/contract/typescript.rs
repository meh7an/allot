//! Emits the module the GNOME extension and the Plasma widget decode `GET /ui/state` and the
//! kill and remove responses with, from the same JSON Schemas as `contract/*.schema.json`.
//!
//! The module is plain JavaScript with JSDoc types, since QML JavaScript cannot run
//! TypeScript and GJS would need a build step for it; `tsc --checkJs` checks it instead. Its
//! decode functions check every field against a table of the types and fill the defaults
//! serde leaves out, so the decoded value matches its JSDoc type exactly.

use std::fmt::Write;

use serde_json::{Map, Value};

use super::schema::{
    Absent, Contract, Primitive, Property, PropertyType, Shape, Type, enum_value, markdown_doc,
    pascal_case, properties, shape, tag_property, tag_value,
};

const HEADER: &str = "\
// Generated from the Rust types behind contract/*.schema.json by
// shared/src/contract/typescript.rs. Do not edit; rerun
// `ALLOT_UPDATE_CONTRACT=1 cargo test -p allot-shared contract` instead.
//
// ES2016 with JSDoc types, so GJS and QML import it as is and tsc checks it. Decode parsed
// JSON with the decode functions at the end: they check every field, fill the defaults allotd
// leaves out, and drop members they do not know, as serde ignores them.
";

const SPEC_TYPES: &str = "
/**
 * How a value is checked: a primitive or definition `type`, or an `array` of items, with
 * whether it may be `null` and, for an optional field, the value a missing field takes.
 *
 * @typedef {({ type: string } | { array: ValueSpec }) & FieldOptions} ValueSpec
 */

/**
 * @typedef {object} FieldOptions
 * @property {boolean} [nullable]
 * @property {null | boolean | string | never[]} [absent]
 */

/** @typedef {{ fields: Readonly<Record<string, ValueSpec>> }} RecordSpec */

/**
 * @typedef {(
 *   | RecordSpec
 *   | { values: readonly string[] }
 *   | { tag: string, variants: Readonly<Record<string, RecordSpec>> }
 * )} DefinitionSpec
 */
";

const RUNTIME: &str = r#"
/** Thrown when a value does not match the contract; the message starts with its path. */
export class ContractError extends Error {
  /**
   * @param {string} path
   * @param {string} problem
   */
  constructor(path, problem) {
    super(`${path}: ${problem}`);
    this.name = "ContractError";
    this.path = path;
  }
}

/** @type {Readonly<Record<"int64" | "uint64" | "uint32", readonly [number, number]>>} */
const INTEGER_RANGES = {
  int64: [-Number.MAX_SAFE_INTEGER, Number.MAX_SAFE_INTEGER],
  uint64: [0, Number.MAX_SAFE_INTEGER],
  uint32: [0, 4294967295],
};

/**
 * @param {object} object
 * @param {string} key
 * @returns {boolean}
 */
function hasOwn(object, key) {
  return Object.prototype.hasOwnProperty.call(object, key);
}

/**
 * The value under `key`, unless `key` is missing or only inherited, such as `constructor`.
 *
 * @template T
 * @param {Readonly<Record<string, T>>} record
 * @param {string} key
 * @returns {T | undefined}
 */
function own(record, key) {
  return hasOwn(record, key) ? record[key] : undefined;
}

/**
 * @param {unknown} value
 * @param {ValueSpec} spec
 * @param {string} path
 * @returns {unknown}
 */
function decodeValue(value, spec, path) {
  if (value === null && spec.nullable) {
    return null;
  }
  if ("array" in spec) {
    if (!Array.isArray(value)) {
      throw new ContractError(path, "expected an array");
    }
    const item = spec.array;
    return value.map((element, index) => decodeValue(element, item, `${path}[${index}]`));
  }
  return decodeType(value, spec.type, path);
}

/**
 * @param {unknown} value
 * @param {string} type
 * @param {string} path
 * @returns {unknown}
 */
function decodeType(value, type, path) {
  switch (type) {
    case "boolean":
    case "string":
      if (typeof value !== type) {
        throw new ContractError(path, `expected a ${type}`);
      }
      return value;
    case "int64":
    case "uint64":
    case "uint32": {
      const [minimum, maximum] = INTEGER_RANGES[type];
      if (
        typeof value !== "number" ||
        !Number.isInteger(value) ||
        value < minimum ||
        value > maximum
      ) {
        throw new ContractError(path, `expected an integer from ${minimum} to ${maximum}`);
      }
      return value;
    }
    case "float":
    case "double":
      if (typeof value !== "number" || !isFinite(value)) {
        throw new ContractError(path, "expected a number");
      }
      return value;
    default: {
      const definition = own(TYPES, type);
      if (definition === undefined) {
        throw new ContractError(path, `no type ${type} in the contract`);
      }
      return decodeDefinition(value, definition, path);
    }
  }
}

/**
 * @param {unknown} value
 * @param {DefinitionSpec} definition
 * @param {string} path
 * @returns {unknown}
 */
function decodeDefinition(value, definition, path) {
  if ("values" in definition) {
    if (typeof value !== "string" || definition.values.indexOf(value) < 0) {
      throw new ContractError(path, `unknown value ${JSON.stringify(value)}`);
    }
    return value;
  }
  const object = decodeObject(value, path);
  if ("tag" in definition) {
    const tag = object[definition.tag];
    const variant = typeof tag === "string" ? own(definition.variants, tag) : undefined;
    if (variant === undefined) {
      throw new ContractError(
        `${path}.${definition.tag}`,
        `unknown variant ${JSON.stringify(tag)}`,
      );
    }
    /** @type {Record<string, unknown>} */
    const decoded = {};
    decoded[definition.tag] = tag;
    return Object.assign(decoded, decodeFields(object, variant, path));
  }
  return decodeFields(object, definition, path);
}

/**
 * @param {unknown} value
 * @param {string} path
 * @returns {Record<string, unknown>}
 */
function decodeObject(value, path) {
  if (typeof value !== "object" || value === null || Array.isArray(value)) {
    throw new ContractError(path, "expected an object");
  }
  return /** @type {Record<string, unknown>} */ (value);
}

/**
 * @param {Record<string, unknown>} object
 * @param {RecordSpec} record
 * @param {string} path
 * @returns {Record<string, unknown>}
 */
function decodeFields(object, record, path) {
  /** @type {Record<string, unknown>} */
  const decoded = {};
  for (const name of Object.keys(record.fields)) {
    // Object.keys lists only names the record has.
    const spec = /** @type {ValueSpec} */ (record.fields[name]);
    const fieldPath = `${path}.${name}`;
    if (hasOwn(object, name)) {
      decoded[name] = decodeValue(object[name], spec, fieldPath);
    } else if ("absent" in spec) {
      decoded[name] = Array.isArray(spec.absent) ? [] : spec.absent;
    } else {
      throw new ContractError(fieldPath, "missing");
    }
  }
  return decoded;
}
"#;

const MAX_WIDTH: usize = 100;

/// Returns the module source for `roots` (the `UiState` schema first) and every definition
/// they use, each type once.
pub(super) fn emit(roots: &[&Value]) -> String {
    let contract = Contract::new(roots);
    let definitions = &contract.definitions;
    let mut out = HEADER.to_owned();
    for (name, schema) in contract.types() {
        match shape(schema) {
            Shape::Enum(variants) => emit_enum_type(&mut out, name, schema, variants),
            Shape::Union(variants) => {
                emit_union_type(&mut out, definitions, name, schema, variants);
            }
            Shape::Record => emit_record_type(&mut out, definitions, name, schema, None, name),
            Shape::Inline => {}
        }
    }
    out.push_str(SPEC_TYPES);

    out.push_str("\n/** @type {Readonly<Record<string, DefinitionSpec>>} */\nconst TYPES = {\n");
    for (name, schema) in contract.types() {
        match shape(schema) {
            Shape::Enum(variants) => {
                let values: Vec<String> = variants
                    .iter()
                    .map(|variant| format!("\"{}\"", enum_value(variant)))
                    .collect();
                writeln!(out, "  {name}: {{ values: [{}] }},", values.join(", ")).unwrap();
            }
            Shape::Union(variants) => {
                let tag = tag_property(name, variants);
                writeln!(out, "  {name}: {{\n    tag: \"{tag}\",\n    variants: {{").unwrap();
                for variant in variants {
                    let fields = properties(definitions, name, variant, Some(tag));
                    write_record_spec(&mut out, tag_value(variant, tag), &fields, "      ");
                }
                out.push_str("    },\n  },\n");
            }
            Shape::Record => {
                let fields = properties(definitions, name, schema, None);
                write_record_spec(&mut out, name, &fields, "  ");
            }
            Shape::Inline => {}
        }
    }
    out.push_str("};\n");

    out.push_str(RUNTIME);
    for name in contract.root_names() {
        writeln!(
            out,
            "
/**
 * Decodes a parsed `{name}` body.
 *
 * @param {{unknown}} json The value `JSON.parse` returned.
 * @returns {{{name}}}
 * @throws {{ContractError}} When the value does not match the contract.
 */
export function decode{name}(json) {{
  return /** @type {{{name}}} */ (decodeType(json, \"{name}\", \"{name}\"));
}}"
        )
        .unwrap();
    }
    out
}

fn emit_enum_type(out: &mut String, name: &str, schema: &Value, variants: &[Value]) {
    let mut doc = markdown_doc(schema, "null").unwrap_or_default();
    doc.push('\n');
    for variant in variants {
        let value = enum_value(variant);
        let variant_doc = markdown_doc(variant, "null").unwrap_or_default();
        write!(doc, "\n- `{value}`").unwrap();
        for (index, line) in variant_doc.lines().enumerate() {
            let separator = if index == 0 { ": " } else { "\n  " };
            write!(doc, "{separator}{line}").unwrap();
        }
    }
    let values: Vec<String> = variants
        .iter()
        .map(|variant| format!("\"{}\"", enum_value(variant)))
        .collect();
    write_typedef(out, &doc, &union_of(&values, name), name, &[]);
}

/// A serde internally tagged enum: a union of one object type per variant, told apart by
/// the tag.
fn emit_union_type(
    out: &mut String,
    definitions: &Map<String, Value>,
    name: &str,
    schema: &Value,
    variants: &[Value],
) {
    let tag = tag_property(name, variants);
    let variant_names: Vec<String> = variants
        .iter()
        .map(|variant| format!("{name}{}", pascal_case(tag_value(variant, tag))))
        .collect();
    let doc = markdown_doc(schema, "null").unwrap_or_default();
    write_typedef(out, &doc, &union_of(&variant_names, name), name, &[]);
    for (variant, variant_name) in variants.iter().zip(&variant_names) {
        emit_record_type(out, definitions, variant_name, variant, Some(tag), name);
    }
}

/// An object typedef; `tag` is a union's tag property, typed as the variant's value.
fn emit_record_type(
    out: &mut String,
    definitions: &Map<String, Value>,
    typedef: &str,
    schema: &Value,
    tag: Option<&str>,
    record: &str,
) {
    let mut lines = Vec::new();
    if let Some(tag) = tag {
        lines.push(format!(
            "@property {{\"{}\"}} {tag}",
            tag_value(schema, tag)
        ));
    }
    for property in properties(definitions, record, schema, tag) {
        let mut line = format!(
            "@property {{{}}} {}",
            render(&property.ty),
            property.json_name
        );
        // The description starts on its own line, so wrapped rustdoc lines stay in width.
        if let Some(doc) = markdown_doc(property.schema, "null") {
            for doc_line in doc.lines() {
                write!(line, "\n  {doc_line}").unwrap();
            }
        }
        lines.push(line);
    }
    let doc = markdown_doc(schema, "null").unwrap_or_default();
    write_typedef(out, &doc, "object", typedef, &lines);
}

/// Writes a JSDoc block: the description, then `@typedef {ty} name`, then `tags`.
fn write_typedef(out: &mut String, doc: &str, ty: &str, name: &str, tags: &[String]) {
    out.push_str("\n/**\n");
    if !doc.is_empty() {
        write_doc_lines(out, doc);
        out.push_str(" *\n");
    }
    write_doc_lines(out, &format!("@typedef {{{ty}}} {name}"));
    for tag in tags {
        write_doc_lines(out, tag);
    }
    out.push_str(" */\n");
}

fn write_doc_lines(out: &mut String, text: &str) {
    assert!(
        !text.contains("*/"),
        "a doc comment cannot contain */: {text}"
    );
    for line in text.lines() {
        if line.is_empty() {
            out.push_str(" *\n");
        } else {
            writeln!(out, " * {line}").unwrap();
        }
    }
}

/// `a | b | c` when the typedef line fits, otherwise one member per line.
fn union_of(members: &[String], name: &str) -> String {
    let one_line = members.join(" | ");
    if format!(" * @typedef {{{one_line}}} {name}").len() <= MAX_WIDTH {
        return one_line;
    }
    let mut lines = String::from("(\n");
    for member in members {
        writeln!(lines, "  | {member}").unwrap();
    }
    lines.push(')');
    lines
}

fn write_record_spec(out: &mut String, key: &str, fields: &[Property], indent: &str) {
    writeln!(out, "{indent}{key}: {{\n{indent}  fields: {{").unwrap();
    for field in fields {
        writeln!(
            out,
            "{indent}    {}: {},",
            field.json_name,
            spec(&field.ty, &field.absent)
        )
        .unwrap();
    }
    writeln!(out, "{indent}  }},\n{indent}}},").unwrap();
}

/// The `ValueSpec` literal the decoder checks a value against.
fn spec(property_type: &PropertyType, absent: &Absent) -> String {
    let mut parts = vec![match &property_type.ty {
        Type::Primitive(primitive) => format!("type: \"{}\"", spec_primitive(*primitive)),
        Type::Named(name) => format!("type: \"{name}\""),
        Type::Array(item) => format!("array: {}", spec(item, &Absent::Fail)),
    }];
    if property_type.nullable {
        parts.push("nullable: true".to_owned());
    }
    let absent = match absent {
        Absent::Fail => None,
        Absent::Null => Some("null".to_owned()),
        Absent::EmptyString => Some("\"\"".to_owned()),
        Absent::False => Some("false".to_owned()),
        Absent::EmptyArray => Some("[]".to_owned()),
        Absent::Variant(value) => Some(format!("\"{value}\"")),
    };
    if let Some(absent) = absent {
        parts.push(format!("absent: {absent}"));
    }
    format!("{{ {} }}", parts.join(", "))
}

fn spec_primitive(primitive: Primitive) -> &'static str {
    match primitive {
        Primitive::Bool => "boolean",
        Primitive::String => "string",
        Primitive::Int64 => "int64",
        Primitive::UInt64 => "uint64",
        Primitive::UInt32 => "uint32",
        Primitive::Float => "float",
        Primitive::Double => "double",
    }
}

/// The JSDoc spelling of a decoded value's type.
fn render(property_type: &PropertyType) -> String {
    let name = match &property_type.ty {
        Type::Primitive(Primitive::Bool) => "boolean".to_owned(),
        Type::Primitive(Primitive::String) => "string".to_owned(),
        Type::Primitive(_) => "number".to_owned(),
        Type::Named(name) => (*name).to_owned(),
        Type::Array(item) if item.nullable => format!("({})[]", render(item)),
        Type::Array(item) => format!("{}[]", render(item)),
    };
    if property_type.nullable {
        format!("{name} | null")
    } else {
        name
    }
}
