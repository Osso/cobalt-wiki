use crate::FormError;

const OPEN: &str = "[[form]]";
const CLOSE: &str = "[[/form]]";

/// Exact text outside one form block and the unmodified YAML inside its markers.
#[derive(Debug, PartialEq)]
pub struct TemplateParts {
    pub body: String,
    pub definition: Option<String>,
}

/// Separate one literal `[[form]]...[[/form]]` block without reformatting text.
///
/// No markers means an ordinary template. Stray, reversed, nested, or repeated
/// markers are errors; this function does not parse the surrounding wiki syntax.
pub fn split_template(template: &str) -> Result<TemplateParts, FormError> {
    let opening = template
        .match_indices(OPEN)
        .map(|(index, _)| index)
        .collect::<Vec<_>>();
    let closing = template
        .match_indices(CLOSE)
        .map(|(index, _)| index)
        .collect::<Vec<_>>();
    match (opening.as_slice(), closing.as_slice()) {
        ([], []) => Ok(TemplateParts {
            body: template.to_owned(),
            definition: None,
        }),
        ([start], [end]) if start < end => {
            let definition = template[start + OPEN.len()..*end].to_owned();
            let body =
                format!("{}{}", &template[..*start], &template[end + CLOSE.len()..]);
            Ok(TemplateParts {
                body,
                definition: Some(definition),
            })
        }
        _ => Err(FormError::Delimiters(
            "expected zero or one ordered opening/closing pair",
        )),
    }
}
