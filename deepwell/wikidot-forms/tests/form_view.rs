use serde_json::{Value as JsonValue, json};
use wikidot_forms::{extract_form_view, parse_values, serialize_values};

const TEMPLATE: &str = "before\n[[form]]\nfields:\n  status:\n    type: select\n    Hint: Choose\n    custom: {enabled: true}\n    values:\n      @@: null\n      active: Active\n      7: Seven\n  notes:\n    type: wiki\n[[/form]]\nafter";

#[test]
fn json_payload_preserves_schema_order_properties_and_scalar_values() {
    let source = "status: @@\nnotes: '[[include example]]'\nunknown: null\nflag: true\ncount: 7\nratio: 1.5\nquoted: '007'\n";
    let view = extract_form_view(TEMPLATE, source).unwrap().unwrap();
    let encoded = serde_json::to_string(&view).unwrap();
    let decoded: JsonValue = serde_json::from_str(&encoded).unwrap();
    assert_eq!(
        decoded,
        json!({
            "schema": {
                "fields": [
                    {"name": "status", "kind": "select",
                     "properties": {"Hint": "Choose", "custom": {"enabled": true}},
                     "options": [{"code": "@@", "label": null},
                                 {"code": "active", "label": "Active"},
                                 {"code": 7, "label": "Seven"}]},
                    {"name": "notes", "kind": "wiki", "properties": {}, "options": []}
                ],
                "properties": {}
            },
            "values": {"status": "@@", "notes": "[[include example]]", "unknown": null,
                       "flag": true, "count": 7, "ratio": 1.5, "quoted": "007"}
        })
    );
    let returned_values = parse_values(&decoded["values"].to_string()).unwrap();
    assert_eq!(returned_values, view.values);
    assert_eq!(
        parse_values(&serialize_values(&returned_values).unwrap()).unwrap(),
        view.values
    );
}

#[test]
fn ordinary_template_has_no_payload_without_parsing_page_yaml() {
    assert!(
        extract_form_view("ordinary wiki template", "not: [valid YAML")
            .unwrap()
            .is_none()
    );
}

#[test]
fn malformed_templates_and_values_fail_explicitly() {
    for template in ["[[form]]", "[[/form]]", "[[form]]fields: [][[/form]]"] {
        assert!(extract_form_view(template, "{}").is_err(), "{template}");
    }
    for source in [
        "broken: [",
        "- item",
        "nested: [item]",
        "7: value",
        "same: one\nsame: two",
    ] {
        assert!(extract_form_view(TEMPLATE, source).is_err(), "{source}");
    }
}
