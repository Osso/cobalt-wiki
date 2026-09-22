//! Pure parsing for Wikidot form definitions and stored scalar field mappings.
//!
//! Schema fields/options retain input order. Uninterpreted properties retain their
//! original keys (including `Hint`) and YAML values. Value round trips preserve
//! decoded scalar types/content, not YAML comments, quoting, or byte formatting.
//! This crate performs no I/O, rendering, permission checks, or editor selection.

mod error;
mod legacy;
mod schema;
mod template;
mod values;
mod view;

pub use error::FormError;
pub use legacy::normalize_legacy_yaml;
pub use schema::{FieldKind, FormField, FormSchema, SelectOption, parse_schema};
pub use serde_yaml_ng::{Mapping, Value};
pub use template::{TemplateParts, split_template};
pub use values::{parse_values, serialize_values};
pub use view::{FormView, extract_form_view};
