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

/// Serialize a scalar field mapping without dropping or coercing field values.
///
/// Output uses normal YAML formatting, not the original comments/quote style.
pub fn serialize_values(values: &Mapping) -> Result<String, FormError> {
    validate_values(values)?;
    serde_yaml_ng::to_string(values).map_err(|source| FormError::Yaml {
        context: "stored field mapping",
        source,
    })
}
