use serde::Serialize;

use crate::{FormError, FormSchema, Mapping, parse_schema, parse_values, split_template};

/// Standalone JSON-ready schema and full-page scalar values; no rendering or I/O.
#[derive(Debug, PartialEq, Serialize)]
pub struct FormView {
    pub schema: FormSchema,
    pub values: Mapping,
}

/// Extract a form view from a template and an entire YAML page record.
///
/// Ordinary templates return None without interpreting page source. Malformed
/// delimiters, definitions, or scalar mappings return explicit parsing errors.
/// Stored field keys are validated as strings; unknown values retain their types.
pub fn extract_form_view(
    template: &str,
    page_yaml: &str,
) -> Result<Option<FormView>, FormError> {
    let parts = split_template(template)?;
    let Some(definition) = parts.definition else {
        return Ok(None);
    };
    let schema = parse_schema(&definition)?;
    let values = parse_values(page_yaml)?;
    Ok(Some(FormView { schema, values }))
}
