use serde::Serialize;

use crate::values::{is_scalar, parse_mapping, require_name};
use crate::{FormError, Mapping, Value};

/// Ordered fields and all other root YAML properties, without renamed keys.
#[derive(Debug, PartialEq, Serialize)]
pub struct FormSchema {
    pub fields: Vec<FormField>,
    pub properties: Mapping,
}

/// A named field: its type/options are explicit; remaining properties stay raw.
#[derive(Debug, PartialEq, Serialize)]
pub struct FormField {
    pub name: String,
    pub kind: FieldKind,
    pub properties: Mapping,
    pub options: Vec<SelectOption>,
}

/// The four field types observed in the Cobalt form definitions.
#[derive(Debug, PartialEq, Serialize)]
#[serde(rename_all = "lowercase")]
pub enum FieldKind {
    Static,
    Text,
    Select,
    Wiki,
}

/// An ordered select entry. Scalar codes/labels are not coerced into strings.
#[derive(Debug, PartialEq, Serialize)]
pub struct SelectOption {
    pub code: Value,
    pub label: Value,
}

fn parse_kind(value: Option<Value>, name: &str) -> Result<FieldKind, FormError> {
    match value {
        Some(Value::String(kind)) => match kind.as_str() {
            "static" => Ok(FieldKind::Static),
            "text" => Ok(FieldKind::Text),
            "select" => Ok(FieldKind::Select),
            "wiki" => Ok(FieldKind::Wiki),
            _ => Err(FormError::Shape(format!(
                "field {name:?} has unsupported type {kind:?}"
            ))),
        },
        _ => Err(FormError::Shape(format!(
            "field {name:?} must have a string type"
        ))),
    }
}

fn parse_options(
    value: Option<Value>,
    name: &str,
) -> Result<Vec<SelectOption>, FormError> {
    let Some(Value::Mapping(options)) = value else {
        return Err(FormError::Shape(format!(
            "select field {name:?} must have a values mapping"
        )));
    };
    options
        .into_iter()
        .map(|(code, label)| {
            if !is_scalar(&code) || !is_scalar(&label) {
                return Err(FormError::Shape(format!(
                    "select field {name:?} codes and labels must be scalars"
                )));
            }
            Ok(SelectOption { code, label })
        })
        .collect()
}

fn validate_properties(properties: &Mapping, context: &str) -> Result<(), FormError> {
    for (key, value) in properties {
        let key = require_name(key, context)?;
        let known_scalar = matches!(
            key.as_str(),
            "label"
                | "after"
                | "hint"
                | "Hint"
                | "width"
                | "height"
                | "default"
                | "value"
        );
        if known_scalar && !is_scalar(value) {
            return Err(FormError::Shape(format!(
                "{context} property {key:?} must be a scalar"
            )));
        }
    }
    Ok(())
}

fn parse_field(name: Value, definition: Value) -> Result<FormField, FormError> {
    let name = require_name(&name, "schema fields")?;
    let Value::Mapping(mut properties) = definition else {
        return Err(FormError::Shape(format!(
            "field {name:?} definition must be a mapping"
        )));
    };
    let kind = parse_kind(properties.remove("type"), &name)?;
    let options = if kind == FieldKind::Select {
        parse_options(properties.remove("values"), &name)?
    } else {
        Vec::new()
    };
    validate_properties(&properties, &format!("field {name:?}"))?;
    Ok(FormField {
        name,
        kind,
        properties,
        options,
    })
}

/// Parse the YAML inside a form block into an ordered, serializable schema.
///
/// Known scalar properties retain their original YAML types and capitalization.
/// Unrecognized properties are preserved, not interpreted. Non-string property
/// names, unsupported field types and malformed/select collection shapes fail.
pub fn parse_schema(definition: &str) -> Result<FormSchema, FormError> {
    let mut properties = parse_mapping(definition, "form schema")?;
    let Some(Value::Mapping(fields)) = properties.remove("fields") else {
        return Err(FormError::Shape(
            "form schema must have a fields mapping".into(),
        ));
    };
    for key in properties.keys() {
        require_name(key, "form schema")?;
    }
    let fields = fields
        .into_iter()
        .map(|(name, definition)| parse_field(name, definition))
        .collect::<Result<_, _>>()?;
    Ok(FormSchema { fields, properties })
}
