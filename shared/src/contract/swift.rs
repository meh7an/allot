//! Emits the Swift types the macOS app decodes `GET /ui/state` and the kill and remove
//! responses with, from the same JSON Schemas as `contract/*.schema.json`.
//!
//! Records are `Codable`, `Sendable` and `Equatable` structs with explicit snake_case coding
//! keys, so a plain `JSONDecoder` reads them. A field serde may leave out decodes to its Rust
//! default instead of becoming optional.

use std::fmt::Write;

use serde_json::{Map, Value};

use super::schema::{
    Absent, Contract, Primitive, Property, PropertyType, Shape, Type, camel_case, enum_value,
    markdown_doc, pascal_case, properties, shape, tag_property, tag_value,
};

const HEADER: &str = "\
// Generated from the Rust types behind contract/*.schema.json by
// shared/src/contract/swift.rs. Do not edit; rerun
// `ALLOT_UPDATE_CONTRACT=1 cargo test -p allot-shared contract` instead.
";

const CONFORMANCES: &str = "Codable, Sendable, Equatable";

const MAX_WIDTH: usize = 100;

// Words a property or case name cannot use without backticks.
const KEYWORDS: &str = "\
    as break case catch class continue default defer do else enum extension false fallthrough \
    for func guard if import in init inout internal is let nil operator private protocol public \
    repeat return self static struct subscript super switch throw throws true try var where while";

/// Returns the Swift source for `roots` (the `UiState` schema first) and every definition they
/// use, each type once.
pub(super) fn emit(roots: &[&Value]) -> String {
    let contract = Contract::new(roots);
    let mut out = HEADER.to_owned();
    for (name, schema) in contract.types() {
        match shape(schema) {
            Shape::Enum(variants) => emit_enum(&mut out, name, schema, variants),
            Shape::Union(variants) => {
                emit_union(&mut out, &contract.definitions, name, schema, variants);
            }
            Shape::Record => emit_struct(&mut out, &contract.definitions, name, schema, None, ""),
            Shape::Inline => {}
        }
    }
    out
}

fn emit_enum(out: &mut String, name: &str, schema: &Value, variants: &[Value]) {
    out.push('\n');
    write_doc(out, "", schema);
    writeln!(out, "public enum {name}: String, {CONFORMANCES} {{").unwrap();
    for variant in variants {
        let value = enum_value(variant);
        write_doc(out, "    ", variant);
        writeln!(out, "    case {}", coding_case(value)).unwrap();
    }
    out.push_str("}\n");
}

/// A serde internally tagged enum: one case per variant, each holding a nested struct of the
/// variant's other fields, told apart by the tag.
fn emit_union(
    out: &mut String,
    definitions: &Map<String, Value>,
    name: &str,
    schema: &Value,
    variants: &[Value],
) {
    let tag = tag_property(name, variants);
    let tag_values: Vec<&str> = variants
        .iter()
        .map(|variant| tag_value(variant, tag))
        .collect();

    out.push('\n');
    write_doc(out, "", schema);
    writeln!(out, "public enum {name}: {CONFORMANCES} {{").unwrap();
    for (variant, value) in variants.iter().zip(&tag_values) {
        write_doc(out, "    ", variant);
        writeln!(
            out,
            "    case {}({})",
            identifier(&camel_case(value)),
            pascal_case(value)
        )
        .unwrap();
    }
    for (variant, value) in variants.iter().zip(&tag_values) {
        emit_struct(
            out,
            definitions,
            &pascal_case(value),
            variant,
            Some(tag),
            "    ",
        );
    }

    let tag_case = identifier(&camel_case(tag));
    writeln!(
        out,
        "
    private enum TagCodingKeys: String, CodingKey {{
        case {}
    }}

    public init(from decoder: any Decoder) throws {{
        let container = try decoder.container(keyedBy: TagCodingKeys.self)
        let tag = try container.decode(String.self, forKey: .{tag_case})
        switch tag {{",
        coding_case(tag)
    )
    .unwrap();
    for value in &tag_values {
        writeln!(
            out,
            "        case \"{value}\": self = try .{}({}(from: decoder))",
            identifier(&camel_case(value)),
            pascal_case(value)
        )
        .unwrap();
    }
    writeln!(
        out,
        "        default:
            throw DecodingError.dataCorruptedError(
                forKey: .{tag_case},
                in: container,
                debugDescription: \"unknown {name} {tag} \\(tag)\"
            )
        }}
    }}

    public func encode(to encoder: any Encoder) throws {{
        var container = encoder.container(keyedBy: TagCodingKeys.self)
        switch self {{"
    )
    .unwrap();
    for value in &tag_values {
        writeln!(
            out,
            "        case .{}(let variant):
            try container.encode(\"{value}\", forKey: .{tag_case})
            try variant.encode(to: encoder)",
            identifier(&camel_case(value))
        )
        .unwrap();
    }
    out.push_str("        }\n    }\n}\n");
}

/// A struct with its coding keys, and a decoding initializer when a missing field takes a
/// default; `skip` is a union's tag property, which the union reads and writes.
fn emit_struct(
    out: &mut String,
    definitions: &Map<String, Value>,
    name: &str,
    schema: &Value,
    skip: Option<&str>,
    indent: &str,
) {
    let properties = properties(definitions, name, schema, skip);
    let inner = format!("{indent}    ");

    out.push('\n');
    write_doc(out, indent, schema);
    writeln!(out, "{indent}public struct {name}: {CONFORMANCES} {{").unwrap();
    for (index, property) in properties.iter().enumerate() {
        if index > 0 {
            out.push('\n');
        }
        write_doc(out, &inner, property.schema);
        writeln!(
            out,
            "{inner}public let {}: {}",
            identifier(&camel_case(property.json_name)),
            render(&property.ty)
        )
        .unwrap();
    }

    writeln!(
        out,
        "\n{inner}private enum CodingKeys: String, CodingKey {{"
    )
    .unwrap();
    for property in &properties {
        writeln!(out, "{inner}    case {}", coding_case(property.json_name)).unwrap();
    }
    writeln!(out, "{inner}}}").unwrap();

    let has_defaults = properties
        .iter()
        .any(|property| !matches!(property.absent, Absent::Fail | Absent::Null));
    if has_defaults {
        emit_decoding_init(out, &properties, &inner);
    }
    writeln!(out, "{indent}}}").unwrap();
}

/// The synthesized decoder requires every key of a non-optional field, so a struct whose
/// fields have defaults decodes by hand.
fn emit_decoding_init(out: &mut String, properties: &[Property], indent: &str) {
    writeln!(
        out,
        "
{indent}public init(from decoder: any Decoder) throws {{
{indent}    let container = try decoder.container(keyedBy: CodingKeys.self)"
    )
    .unwrap();
    for property in properties {
        let member = identifier(&camel_case(property.json_name));
        let wrapped = type_name(&property.ty.ty);
        let call = if property.ty.nullable || !matches!(property.absent, Absent::Fail) {
            "decodeIfPresent"
        } else {
            "decode"
        };
        let fallback = match property.absent {
            Absent::Fail | Absent::Null => String::new(),
            Absent::EmptyString => " ?? \"\"".to_owned(),
            Absent::False => " ?? false".to_owned(),
            Absent::EmptyArray => " ?? []".to_owned(),
            Absent::Variant(value) => format!(" ?? .{}", identifier(&camel_case(value))),
        };
        let target = format!("{indent}    self.{member} =");
        let value = format!("try container.{call}({wrapped}.self, forKey: .{member}){fallback}");
        if target.len() + 1 + value.len() <= MAX_WIDTH {
            writeln!(out, "{target} {value}").unwrap();
        } else {
            writeln!(out, "{target}\n{indent}        {value}").unwrap();
        }
    }
    writeln!(out, "{indent}}}").unwrap();
}

/// The Swift spelling of a property type, optional when the schema allows `null`.
fn render(property_type: &PropertyType) -> String {
    let name = type_name(&property_type.ty);
    if property_type.nullable {
        format!("{name}?")
    } else {
        name
    }
}

fn type_name(ty: &Type) -> String {
    match ty {
        Type::Primitive(primitive) => primitive_name(*primitive).to_owned(),
        Type::Named(name) => (*name).to_owned(),
        Type::Array(item) => format!("[{}]", render(item)),
    }
}

fn primitive_name(primitive: Primitive) -> &'static str {
    match primitive {
        Primitive::Bool => "Bool",
        Primitive::String => "String",
        Primitive::Int64 => "Int64",
        Primitive::UInt64 => "UInt64",
        Primitive::UInt32 => "UInt32",
        Primitive::Float => "Float",
        Primitive::Double => "Double",
    }
}

/// A coding key or raw-valued case: the camelCase name, with the JSON name as its raw value
/// when the two differ.
fn coding_case(json_name: &str) -> String {
    let case = camel_case(json_name);
    if case == json_name {
        identifier(&case)
    } else {
        format!("{} = \"{json_name}\"", identifier(&case))
    }
}

fn identifier(name: &str) -> String {
    if KEYWORDS.split_whitespace().any(|keyword| keyword == name) {
        format!("`{name}`")
    } else {
        name.to_owned()
    }
}

/// Writes the schema's `description` as a `///` doc comment.
fn write_doc(out: &mut String, indent: &str, schema: &Value) {
    let Some(doc) = markdown_doc(schema, "nil") else {
        return;
    };
    for line in doc.lines() {
        if line.is_empty() {
            writeln!(out, "{indent}///").unwrap();
        } else {
            writeln!(out, "{indent}/// {line}").unwrap();
        }
    }
}
