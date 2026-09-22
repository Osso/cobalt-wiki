use wikidot_forms::{FormError, FormView, extract_form_view};

pub(crate) fn template_slug(page_slug: &str) -> Option<String> {
    let (category, name) = match page_slug.split_once(':') {
        Some((category, name)) => (Some(category), name),
        None => (None, page_slug),
    };
    if name == "_template" {
        return None;
    }
    Some(match category {
        Some(category) => format!("{category}:_template"),
        None => "_template".into(),
    })
}

pub(crate) fn extract_page_form(
    template_source: Option<&str>,
    page_source: &str,
) -> Result<Option<FormView>, FormError> {
    match template_source {
        Some(template) => extract_form_view(template, page_source),
        None => Ok(None),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const TEMPLATE: &str =
        "before\n[[form]]\nfields:\n  name:\n    type: text\n[[/form]]\nafter";

    #[test]
    fn resolves_category_and_default_templates_without_self_parsing() {
        assert_eq!(
            template_slug("character:example"),
            Some("character:_template".into())
        );
        assert_eq!(template_slug("start"), Some("_template".into()));
        assert_eq!(template_slug("character:_template"), None);
        assert_eq!(template_slug("_template"), None);
    }

    #[test]
    fn missing_and_ordinary_templates_do_not_parse_page_source() {
        assert!(extract_page_form(None, "not: [yaml").unwrap().is_none());
        assert!(
            extract_page_form(Some("ordinary wiki"), "not: [yaml")
                .unwrap()
                .is_none()
        );
    }

    #[test]
    fn exposes_entire_yaml_record_without_modifying_source() {
        let source = "name: Example\nunknown: true\ncount: 7\n";
        let view = extract_page_form(Some(TEMPLATE), source).unwrap().unwrap();
        let json = serde_json::to_value(view).unwrap();
        assert_eq!(json["schema"]["fields"][0]["name"], "name");
        assert_eq!(
            json["values"],
            serde_json::json!({"name":"Example", "unknown":true, "count":7})
        );
        assert_eq!(source, "name: Example\nunknown: true\ncount: 7\n");
    }

    #[test]
    fn malformed_templates_and_records_are_errors() {
        assert!(extract_page_form(Some("[[form]]"), "name: Example").is_err());
        assert!(
            extract_page_form(Some("[[form]]\nfields: []\n[[/form]]"), "name: Example")
                .is_err()
        );
        assert!(extract_page_form(Some(TEMPLATE), "name: [nested]").is_err());
    }
}
