use wikidot_forms::{Mapping, Value, apply_field_updates, parse_schema, parse_values};

const SCHEMA: &str = r#"
fields:
  heading: {type: static, value: Profile}
  name: {type: text, required: true, default: Someone}
  biography: {type: wiki}
  choice:
    type: select
    values:
      1: Numeric
      '1': Textual
      true: Boolean
      null: Unset
      '': Empty
"#;

fn apply(original: &str, updates: &str) -> Result<Mapping, String> {
    let schema = parse_schema(SCHEMA).unwrap();
    let original = parse_values(original).unwrap();
    let updates = parse_values(updates).unwrap();
    apply_field_updates(&schema, &original, &updates)
        .map(|yaml| parse_values(&yaml).unwrap())
        .map_err(|error| error.to_string())
}

#[test]
fn changes_known_fields_preserving_unknown_fields_and_scalar_types() {
    let actual = apply(
        "name: Before\nbiography: Old\nextra: retained\nflag: true\ncount: 42\nmissing: null\n",
        "name: After\nbiography: |\n  **New**\n  Second line\n",
    ).unwrap();
    let expected = parse_values(
        "name: After\nbiography: |\n  **New**\n  Second line\nextra: retained\nflag: true\ncount: 42\nmissing: null\n",
    ).unwrap();
    assert_eq!(actual, expected);
    assert_eq!(
        actual.keys().collect::<Vec<_>>(),
        expected.keys().collect::<Vec<_>>()
    );
}

#[test]
fn empty_string_and_null_remain_distinct_without_default_or_required_coercion() {
    for update in ["name: ''", "name: null"] {
        assert_eq!(
            apply("name: Before", update).unwrap(),
            parse_values(update).unwrap()
        );
    }
    assert!(apply("{}", "{}").unwrap().is_empty());
}

#[test]
fn rejects_static_changes_but_accepts_unchanged_static_values() {
    assert!(
        apply("heading: Old", "heading: New")
            .unwrap_err()
            .contains("static")
    );
    assert!(
        apply("{}", "heading: Profile")
            .unwrap_err()
            .contains("static")
    );
    assert_eq!(
        apply("heading: Old", "heading: Old").unwrap(),
        parse_values("heading: Old").unwrap()
    );
}

#[test]
fn rejects_unknown_updates_even_if_the_key_already_exists() {
    for original in ["extra: retained", "{}"] {
        assert!(
            apply(original, "extra: replaced")
                .unwrap_err()
                .contains("unknown")
        );
    }
}

#[test]
fn rejects_nested_and_tagged_updates_and_nonstring_keys() {
    let schema = parse_schema(SCHEMA).unwrap();
    let original = parse_values("name: Before").unwrap();
    for yaml in [
        "name: [nested]",
        "name: {nested: value}",
        "name: !custom value",
        "1: value",
    ] {
        let updates: Mapping = serde_yaml_ng::from_str(yaml).unwrap();
        assert!(
            apply_field_updates(&schema, &original, &updates).is_err(),
            "{yaml}"
        );
    }
    assert_eq!(original, parse_values("name: Before").unwrap());
}

#[test]
fn select_codes_preserve_scalar_equality_including_empty_and_null() {
    for code in ["1", "'1'", "true", "null", "''"] {
        let update = format!("choice: {code}");
        assert_eq!(
            apply("choice: legacy", &update).unwrap(),
            parse_values(&update).unwrap()
        );
    }
    for code in ["'true'", "'null'", "2", "false", "Numeric"] {
        assert!(
            apply("choice: legacy", &format!("choice: {code}"))
                .unwrap_err()
                .contains("option code")
        );
    }
}

#[test]
fn unchanged_unknown_select_code_is_preserved_but_new_invalid_code_fails() {
    for updates in ["{}", "choice: legacy"] {
        assert_eq!(
            apply("choice: legacy", updates).unwrap(),
            parse_values("choice: legacy").unwrap()
        );
    }
    assert!(apply("choice: legacy", "choice: other").is_err());
    assert!(apply("{}", "choice: legacy").is_err());
    // An absent value is not an existing null value.
    let schema =
        parse_schema("fields:\n  choice: {type: select, values: {ok: Yes}}\n").unwrap();
    let updates = parse_values("choice: null").unwrap();
    assert!(apply_field_updates(&schema, &Mapping::new(), &updates).is_err());
}

#[test]
fn legacy_at_signs_round_trip_as_a_whole_mapping() {
    let schema = parse_schema("fields:\n  @@: {type: text}\n  choice:\n    type: select\n    values:\n      @@: Unset\n      ready: Ready\n").unwrap();
    let original = parse_values(
        "@@: Before\nchoice: @@\nunknown: @@\nflag: false\nnumber: 3.5\nempty: null\n",
    )
    .unwrap();
    let updates = parse_values("@@: After\nchoice: ready\n").unwrap();
    let serialized = apply_field_updates(&schema, &original, &updates).unwrap();
    let expected = parse_values(
        "@@: After\nchoice: ready\nunknown: @@\nflag: false\nnumber: 3.5\nempty: null\n",
    )
    .unwrap();
    assert_eq!(parse_values(&serialized).unwrap(), expected);
    assert_eq!(original.get("@@"), Some(&Value::String("Before".into())));
}
