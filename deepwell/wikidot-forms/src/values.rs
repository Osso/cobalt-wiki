use crate::{FormError, Mapping, Value, normalize_legacy_yaml};

pub(crate) fn parse_mapping(
    source: &str,
    context: &'static str,
) -> Result<Mapping, FormError> {
    let normalized = normalize_legacy_yaml(source);
    let value: Value = serde_yaml_ng::from_str(&normalized)
        .map_err(|source| FormError::Yaml { context, source })?;
    let Value::Mapping(mapping) = value else {
        return Err(FormError::Shape(format!("{context} must be a mapping")));
    };
    Ok(mapping)
}

pub(crate) fn is_scalar(value: &Value) -> bool {
    matches!(
        value,
        Value::Null | Value::Bool(_) | Value::Number(_) | Value::String(_)
    )
}

pub(crate) fn require_name(value: &Value, context: &str) -> Result<String, FormError> {
    match value {
        Value::String(name) if !name.is_empty() => Ok(name.clone()),
        _ => Err(FormError::Shape(format!(
            "{context} keys must be nonempty strings"
        ))),
    }
}

fn validate_values(values: &Mapping) -> Result<(), FormError> {
    for (name, value) in values {
        let name = require_name(name, "stored field mapping")?;
        if !is_scalar(value) {
            return Err(FormError::Shape(format!(
                "stored field {name:?} must be a scalar"
            )));
        }
    }
    Ok(())
}

/// Parse named scalar fields, retaining unknown field names, order and YAML types.
///
/// Empty documents, duplicate names, tagged values and nested collections fail
/// explicitly. An explicit empty mapping (`{}`) is valid.
pub fn parse_values(source: &str) -> Result<Mapping, FormError> {
    let values = parse_mapping(source, "stored field mapping")?;
    validate_values(&values)?;
    Ok(values)
}

/// Apply scalar field updates to an entire stored YAML mapping and serialize it.
///
/// Unknown original fields and unchanged scalar types are retained. Static fields
/// cannot change; changed select values must equal a declared option code. No
/// defaults, required-field rules, or empty-value coercions are applied.
pub fn apply_field_updates(
    schema: &crate::FormSchema,
    original: &Mapping,
    updates: &Mapping,
) -> Result<String, FormError> {
    validate_values(original)?;
    let fields: std::collections::HashMap<_, _> = schema
        .fields
        .iter()
        .map(|field| (field.name.as_str(), field))
        .collect();
    let mut values = original.clone();
    for (key, value) in updates {
        let name = require_name(key, "submitted field mapping")?;
        let field = fields.get(name.as_str()).ok_or_else(|| {
            FormError::Shape(format!("unknown submitted field {name:?}"))
        })?;
        validate_update(field, original.get(key), value)?;
        values.insert(key.clone(), value.clone());
    }
    serialize_values(&values)
}

fn validate_update(
    field: &crate::FormField,
    original: Option<&Value>,
    value: &Value,
) -> Result<(), FormError> {
    let name = &field.name;
    if !is_scalar(value) {
        return Err(FormError::Shape(format!(
            "submitted field {name:?} must be a scalar"
        )));
    }
    if original == Some(value) {
        return Ok(());
    }
    match field.kind {
        crate::FieldKind::Static => Err(FormError::Shape(format!(
            "static field {name:?} cannot be changed"
        ))),
        crate::FieldKind::Select => {
            if field.options.iter().any(|option| option.code == *value) {
                return Ok(());
            }
            Err(FormError::Shape(format!(
                "select field {name:?} must match an option code"
            )))
        }
        crate::FieldKind::Text | crate::FieldKind::Wiki => Ok(()),
    }
}

/// Serialize a scalar field mapping as Wikidot writes form records
/// (sfYaml quoting, no trailing newline) without dropping or coercing values.
pub fn serialize_values(values: &Mapping) -> Result<String, FormError> {
    validate_values(values)?;
    Ok(crate::dump::dump_record(values))
}

/// The record Wikidot saves for a new page from its form: every non-static
/// field in schema order, as the posted form text (the submitted value, else
/// the field default, else empty).
pub fn new_record(
    schema: &crate::FormSchema,
    submitted: &Mapping,
) -> Result<String, FormError> {
    for (key, value) in submitted {
        let name = require_name(key, "submitted field mapping")?;
        let field = schema
            .fields
            .iter()
            .find(|field| field.name == name)
            .ok_or_else(|| {
                FormError::Shape(format!("unknown submitted field {name:?}"))
            })?;
        validate_update(field, None, value)?;
    }
    let values = schema
        .fields
        .iter()
        .filter(|field| field.kind != crate::FieldKind::Static)
        .map(|field| {
            let key = Value::String(field.name.clone());
            let value = submitted
                .get(&key)
                .or_else(|| field.properties.get("default"))
                .map_or_else(String::new, posted_text);
            (key, Value::String(value))
        })
        .collect();
    serialize_values(&values)
}

/// A scalar as a browser posts it from a form control.
fn posted_text(value: &Value) -> String {
    match value {
        Value::String(text) => text.clone(),
        Value::Number(number) => number.to_string(),
        Value::Bool(true) => "1".into(),
        _ => String::new(),
    }
}
