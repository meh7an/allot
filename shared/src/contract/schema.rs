//! The JSON Schema shapes the contract uses, read once for every emitter, so the C#, Swift and
//! TypeScript types agree on names, nullability and defaults.
//!
//! Anything outside these shapes panics, so a new shape gets a deliberate mapping in every
//! language instead of a silent catch-all.

use serde_json::{Map, Value};

/// The contract roots (the `UiState` schema first) and every definition they use, each once.
pub(super) struct Contract<'a> {
    pub(super) roots: Vec<(&'a str, &'a Value)>,
    pub(super) definitions: Map<String, Value>,
}

impl<'a> Contract<'a> {
    pub(super) fn new(roots: &[&'a Value]) -> Self {
        let mut definitions = Map::new();
        for root in roots {
            let own = root["definitions"]
                .as_object()
                .expect("the schema has definitions");
            for (name, schema) in own {
                match definitions.get(name) {
                    Some(known) => assert_eq!(known, schema, "{name} differs between the roots"),
                    None => {
                        definitions.insert(name.clone(), schema.clone());
                    }
                }
            }
        }
        let roots: Vec<(&str, &Value)> = roots
            .iter()
            .map(|root| {
                let name = root["title"].as_str().expect("the root schema has a title");
                assert!(
                    !definitions.contains_key(name),
                    "{name} is both a root and a definition"
                );
                (name, *root)
            })
            .collect();
        Self { roots, definitions }
    }

    /// Every type to emit: the roots in order, then the definitions by name.
    pub(super) fn types(&self) -> impl Iterator<Item = (&str, &Value)> {
        self.roots.iter().copied().chain(
            self.definitions
                .iter()
                .map(|(name, schema)| (name.as_str(), schema)),
        )
    }

    pub(super) fn root_names(&self) -> Vec<&'a str> {
        self.roots.iter().map(|(name, _)| *name).collect()
    }
}

/// What a definition becomes in the target language.
pub(super) enum Shape<'a> {
    /// An object with named properties.
    Record,
    /// A unit enum; each variant is a `const` string.
    Enum(&'a [Value]),
    /// A serde internally tagged enum; each variant is an object.
    Union(&'a [Value]),
    /// A newtype such as `LeaseId`, inlined as its primitive wherever it is used.
    Inline,
}

pub(super) fn shape(schema: &Value) -> Shape<'_> {
    if let Some(variants) = schema["oneOf"].as_array() {
        if is_unit_enum(variants) {
            Shape::Enum(variants)
        } else {
            Shape::Union(variants)
        }
    } else if schema.get("properties").is_some() {
        Shape::Record
    } else {
        primitive_of(schema);
        Shape::Inline
    }
}

fn is_unit_enum(variants: &[Value]) -> bool {
    variants
        .iter()
        .all(|variant| variant.get("const").is_some())
}

/// The string serde writes for a unit enum variant.
pub(super) fn enum_value(variant: &Value) -> &str {
    variant["const"].as_str().expect("enum values are strings")
}

#[derive(Clone, Copy, PartialEq, Eq)]
pub(super) enum Primitive {
    Bool,
    String,
    Int64,
    UInt64,
    UInt32,
    Float,
    Double,
}

pub(super) enum Type<'a> {
    Primitive(Primitive),
    /// A record, enum or union definition.
    Named(&'a str),
    Array(Box<PropertyType<'a>>),
}

/// A property's type, with whether the schema allows `null`.
pub(super) struct PropertyType<'a> {
    pub(super) ty: Type<'a>,
    pub(super) nullable: bool,
}

pub(super) fn property_type<'a>(
    definitions: &'a Map<String, Value>,
    schema: &'a Value,
) -> PropertyType<'a> {
    if let Some(name) = reference(schema) {
        let target = &definitions[name];
        let ty = match shape(target) {
            Shape::Inline => Type::Primitive(primitive_of(target)),
            Shape::Record | Shape::Enum(_) | Shape::Union(_) => Type::Named(name),
        };
        return PropertyType {
            ty,
            nullable: false,
        };
    }
    if let Some([only]) = schema["allOf"].as_array().map(Vec::as_slice) {
        return property_type(definitions, only);
    }
    if let Some([inner, null]) = schema["anyOf"].as_array().map(Vec::as_slice)
        && null["type"] == "null"
    {
        return PropertyType {
            nullable: true,
            ..property_type(definitions, inner)
        };
    }
    if let Some([kind, null]) = schema["type"].as_array().map(Vec::as_slice)
        && null == "null"
    {
        return PropertyType {
            ty: Type::Primitive(primitive(kind.as_str().unwrap(), schema["format"].as_str())),
            nullable: true,
        };
    }
    if schema["type"] == "array" {
        return PropertyType {
            ty: Type::Array(Box::new(property_type(definitions, &schema["items"]))),
            nullable: false,
        };
    }
    PropertyType {
        ty: Type::Primitive(primitive_of(schema)),
        nullable: false,
    }
}

fn reference(schema: &Value) -> Option<&str> {
    schema["$ref"].as_str().map(|reference| {
        reference
            .strip_prefix("#/definitions/")
            .unwrap_or_else(|| panic!("unsupported $ref {reference}"))
    })
}

/// Whether `schema` refers to a unit enum, whose default is one of its names.
fn is_enum(definitions: &Map<String, Value>, schema: &Value) -> bool {
    if let Some([only]) = schema["allOf"].as_array().map(Vec::as_slice) {
        return is_enum(definitions, only);
    }
    reference(schema).is_some_and(|name| matches!(shape(&definitions[name]), Shape::Enum(_)))
}

fn primitive_of(schema: &Value) -> Primitive {
    let kind = schema["type"]
        .as_str()
        .unwrap_or_else(|| panic!("no mapping for schema {schema}"));
    primitive(kind, schema["format"].as_str())
}

fn primitive(kind: &str, format: Option<&str>) -> Primitive {
    match (kind, format) {
        ("boolean", None) => Primitive::Bool,
        ("string", None) => Primitive::String,
        ("integer", Some("int64")) => Primitive::Int64,
        ("integer", Some("uint64")) => Primitive::UInt64,
        ("integer", Some("uint32")) => Primitive::UInt32,
        ("number", Some("float")) => Primitive::Float,
        ("number", Some("double")) => Primitive::Double,
        _ => panic!("no mapping for type {kind} with format {format:?}"),
    }
}

/// What a decoder does when a property is missing from the JSON.
pub(super) enum Absent<'a> {
    /// The property is required.
    Fail,
    /// It decodes as `null`.
    Null,
    EmptyString,
    False,
    EmptyArray,
    /// It decodes as this unit enum value.
    Variant(&'a str),
}

pub(super) struct Property<'a> {
    pub(super) json_name: &'a str,
    pub(super) schema: &'a Value,
    pub(super) ty: PropertyType<'a>,
    pub(super) absent: Absent<'a>,
}

impl Property<'_> {
    pub(super) fn is_required(&self) -> bool {
        matches!(self.absent, Absent::Fail)
    }
}

/// The properties of the object `schema` by JSON name; `skip` is a union's tag property, which
/// the union itself reads and writes.
pub(super) fn properties<'a>(
    definitions: &'a Map<String, Value>,
    record: &str,
    schema: &'a Value,
    skip: Option<&str>,
) -> Vec<Property<'a>> {
    let required: Vec<&str> = schema["required"]
        .as_array()
        .map(|names| names.iter().map(|name| name.as_str().unwrap()).collect())
        .unwrap_or_default();
    schema["properties"]
        .as_object()
        .unwrap()
        .iter()
        .filter(|(json_name, _)| Some(json_name.as_str()) != skip)
        .map(|(json_name, property)| {
            let ty = property_type(definitions, property);
            let is_required = required.contains(&json_name.as_str());
            let absent = match (is_required, ty.nullable, property.get("default")) {
                (true, _, None) => Absent::Fail,
                (false, true, None | Some(Value::Null)) => Absent::Null,
                (false, false, Some(Value::String(text)))
                    if text.is_empty() && matches!(ty.ty, Type::Primitive(Primitive::String)) =>
                {
                    Absent::EmptyString
                }
                (false, false, Some(Value::String(text))) if is_enum(definitions, property) => {
                    Absent::Variant(text)
                }
                (false, false, Some(Value::Bool(false)))
                    if matches!(ty.ty, Type::Primitive(Primitive::Bool)) =>
                {
                    Absent::False
                }
                (false, false, Some(Value::Array(items))) if items.is_empty() => Absent::EmptyArray,
                _ => panic!("{record}.{json_name}: no mapping for its required and default mix"),
            };
            Property {
                json_name,
                schema: property,
                ty,
                absent,
            }
        })
        .collect()
}

/// The one property every variant fixes to a `const`, which serde writes as the tag.
pub(super) fn tag_property<'a>(name: &str, variants: &'a [Value]) -> &'a str {
    let first = variants[0]["properties"]
        .as_object()
        .unwrap_or_else(|| panic!("{name}: variants must be objects"));
    first
        .iter()
        .find(|(key, _)| {
            variants
                .iter()
                .all(|variant| variant["properties"][key.as_str()].get("const").is_some())
        })
        .map(|(key, _)| key.as_str())
        .unwrap_or_else(|| panic!("{name}: no tag property shared by every variant"))
}

/// The tag value of each variant of a union whose tag is `tag`.
pub(super) fn tag_value<'a>(variant: &'a Value, tag: &str) -> &'a str {
    variant["properties"][tag]["const"].as_str().unwrap()
}

/// The `description` of `schema` with rustdoc links turned into code spans and `None` shown
/// as `none`, for doc comments that read markdown.
pub(super) fn markdown_doc(schema: &Value, none: &str) -> Option<String> {
    let description = schema["description"].as_str()?;
    Some(
        description
            .replace("[`", "`")
            .replace("`]", "`")
            .replace("`None`", &format!("`{none}`")),
    )
}

pub(super) fn pascal_case(snake: &str) -> String {
    snake
        .split('_')
        .map(|word| {
            let mut chars = word.chars();
            chars.next().map_or_else(String::new, |first| {
                first.to_ascii_uppercase().to_string() + chars.as_str()
            })
        })
        .collect()
}

pub(super) fn camel_case(snake: &str) -> String {
    let pascal = pascal_case(snake);
    let mut chars = pascal.chars();
    chars.next().map_or_else(String::new, |first| {
        first.to_ascii_lowercase().to_string() + chars.as_str()
    })
}
