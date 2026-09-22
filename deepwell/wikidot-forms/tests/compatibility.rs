use wikidot_forms::{
    FieldKind, Value, normalize_legacy_yaml, parse_schema, parse_values,
    serialize_values, split_template,
};

#[test]
fn separates_definition_without_reformatting_surrounding_template() {
    let source =
        "α\r\n[[include Box]]\n[[form]]\nfields: {}\n[[/form]]\r\n%%content%%  \n";
    let parts = split_template(source).unwrap();
    assert_eq!(parts.body, "α\r\n[[include Box]]\n\r\n%%content%%  \n");
    assert_eq!(parts.definition.as_deref(), Some("\nfields: {}\n"));
}

#[test]
fn ordinary_template_is_unchanged() {
    let source = "[[include Box]]\n%%content%%\n";
    let parts = split_template(source).unwrap();
    assert_eq!(parts.body, source);
    assert_eq!(parts.definition, None);
}

#[test]
fn rejects_unmatched_reversed_and_multiple_form_blocks() {
    for source in [
        "prefix [[form]] fields: {}",
        "[[/form]] suffix",
        "[[/form]][[form]]",
        "[[form]][[form]][[/form]]",
        "[[form]]fields: {}[[/form]][[form]]fields: {}[[/form]]",
        "[[form]]fields: {}[[/form]][[/form]]",
    ] {
        assert!(split_template(source).is_err(), "accepted {source:?}");
    }
}

#[test]
fn schema_preserves_field_option_order_and_unrecognized_properties() {
    let schema = parse_schema(
        r#"
version-note: untouched
fields:
  introduction:
    type: static
    label: Instructions
    value: "**Bonjour, 世界**"
    extension: {enabled: true}
  biography:
    type: wiki
    Hint: Keep this spelling
    hint: Keep this too
    height: 15
    width: '70'
    default: |
      [[include ImageBox
      | caption=雪
      ]]
  displayName:
    type: text
    label: Display name
    after: "A character's name"
    width: 40
    default: '001'
  sex:
    label: Body Type
    type: select
    values:
      female: Female
      male: Male
    after: In-game model
"#,
    )
    .unwrap();
    assert_eq!(
        schema
            .fields
            .iter()
            .map(|field| field.name.as_str())
            .collect::<Vec<_>>(),
        ["introduction", "biography", "displayName", "sex"]
    );
    assert_eq!(
        schema.properties["version-note"],
        Value::String("untouched".into())
    );
    assert_eq!(schema.fields[0].kind, FieldKind::Static);
    assert_eq!(
        schema.fields[0].properties["extension"]["enabled"],
        Value::Bool(true)
    );
    assert_eq!(schema.fields[1].kind, FieldKind::Wiki);
    assert_eq!(
        schema.fields[1].properties["Hint"],
        Value::String("Keep this spelling".into())
    );
    assert_eq!(
        schema.fields[1].properties["hint"],
        Value::String("Keep this too".into())
    );
    assert_eq!(
        schema.fields[1].properties["height"],
        Value::Number(15.into())
    );
    assert_eq!(
        schema.fields[1].properties["width"],
        Value::String("70".into())
    );
    assert_eq!(
        schema.fields[1].properties["default"],
        Value::String("[[include ImageBox\n| caption=雪\n]]\n".into())
    );
    assert_eq!(schema.fields[2].kind, FieldKind::Text);
    assert_eq!(
        schema.fields[2].properties["default"],
        Value::String("001".into())
    );
    assert_eq!(schema.fields[3].kind, FieldKind::Select);
    assert_eq!(
        schema.fields[3].options[0].code,
        Value::String("female".into())
    );
    assert_eq!(
        schema.fields[3].options[0].label,
        Value::String("Female".into())
    );
    assert_eq!(
        schema.fields[3].options[1].code,
        Value::String("male".into())
    );
    assert_eq!(
        schema.fields[3].options[1].label,
        Value::String("Male".into())
    );
}

#[test]
fn schema_serializes_for_editor_without_losing_options_or_capitalization() {
    let schema = parse_schema(
        "fields:\n  story:\n    type: wiki\n    Hint: Original\n  hidden:\n    type: select\n    values:\n      @@: No\n      yes: Yes\n    default: @@\n",
    ).unwrap();
    let json = serde_json::to_value(&schema).unwrap();
    assert_eq!(json["fields"][0]["name"], "story");
    assert_eq!(json["fields"][0]["properties"]["Hint"], "Original");
    assert!(json["fields"][0]["properties"].get("hint").is_none());
    assert_eq!(json["fields"][1]["kind"], "select");
    assert_eq!(json["fields"][1]["options"][0]["code"], "@@");
    assert_eq!(json["fields"][1]["options"][0]["label"], "No");
    assert_eq!(json["fields"][1]["options"][1]["code"], "yes");
    assert_eq!(json["fields"][1]["properties"]["default"], "@@");
}

#[test]
fn non_select_values_property_is_retained_not_interpreted() {
    let schema =
        parse_schema("fields:\n  note:\n    type: text\n    values: [custom, data]\n")
            .unwrap();
    assert_eq!(
        schema.fields[0].properties["values"],
        Value::Sequence(vec![
            Value::String("custom".into()),
            Value::String("data".into())
        ])
    );
    assert!(schema.fields[0].options.is_empty());
}

#[test]
fn scalar_select_codes_and_labels_keep_their_yaml_types() {
    let schema = parse_schema("fields:\n  choice:\n    type: select\n    values:\n      7: true\n      '007': false\n      null: None\n").unwrap();
    let options = &schema.fields[0].options;
    assert_eq!(options[0].code, Value::Number(7.into()));
    assert_eq!(options[0].label, Value::Bool(true));
    assert_eq!(options[1].code, Value::String("007".into()));
    assert_eq!(options[1].label, Value::Bool(false));
    assert_eq!(options[2].code, Value::Null);
}

#[test]
fn rejects_malformed_or_unsupported_schema_instead_of_discarding_it() {
    for source in [
        "[]",
        "other: data",
        "fields: []",
        "fields: {bad: []}",
        "fields: {bad: {label: Missing type}}",
        "fields: {bad: {type: 5}}",
        "fields: {bad: {type: date}}",
        "fields: {bad: {type: select}}",
        "fields: {bad: {type: select, values: [x, y]}}",
        "fields: {bad: {type: select, values: {a: [x, y]}}}",
        "fields: {bad: {type: select, values: {? [x, y]: label}}}",
        "fields: {5: {type: text}}",
        "fields: {bad: {type: text, 5: unknown}}",
        "fields: {bad: {type: text, label: [wrong, shape]}}",
        "fields:\n  same: {type: text}\n  same: {type: wiki}\n",
        "fields: {bad: {type: select, values: {x: one, x: two}}}",
        "fields:\n  race:\n    type: select\n    values:\n      orc: Orc:\n",
    ] {
        assert!(parse_schema(source).is_err(), "accepted {source:?}");
    }
    let error = parse_schema("fields: {birthday: {type: date}}")
        .unwrap_err()
        .to_string();
    assert!(error.contains("birthday"));
    assert!(error.contains("date"));
}

#[test]
fn empty_explicit_mappings_are_valid_but_empty_document_is_not() {
    assert!(parse_schema("fields: {}").unwrap().fields.is_empty());
    assert!(parse_values("{}").unwrap().is_empty());
    assert!(parse_values("").is_err());
}

#[test]
fn stored_values_round_trip_scalar_kinds_unicode_and_wiki_markup() {
    let source = "name: 'Élodie 雪'\nhidden: @@\nnumberText: '001'\ncount: 17\nactive: true\nabsent: null\nunknownField: keep me\nrelationships: |\n  [[include RelationshipBox\n  | name=Friend\n  ]]\n";
    let values = parse_values(source).unwrap();
    assert_eq!(values["hidden"], Value::String("@@".into()));
    assert_eq!(values["numberText"], Value::String("001".into()));
    assert_eq!(values["count"], Value::Number(17.into()));
    assert_eq!(values["active"], Value::Bool(true));
    assert_eq!(values["absent"], Value::Null);
    assert_eq!(values["unknownField"], Value::String("keep me".into()));
    assert_eq!(
        values["relationships"],
        Value::String("[[include RelationshipBox\n| name=Friend\n]]\n".into())
    );
    let encoded = serialize_values(&values).unwrap();
    let decoded = parse_values(&encoded).unwrap();
    assert_eq!(decoded, values);
    assert_eq!(
        decoded.keys().collect::<Vec<_>>(),
        values.keys().collect::<Vec<_>>()
    );
}

#[test]
fn rejects_non_scalar_stored_values_and_duplicate_keys() {
    for source in [
        "[]",
        "field: [one, two]",
        "field: {nested: value}",
        "7: value",
        "field: one\nfield: two\n",
        "field: !custom value",
    ] {
        assert!(parse_values(source).is_err(), "accepted {source:?}");
    }
    let invalid = serde_yaml_ng::from_str("field: [one, two]").unwrap();
    assert!(serialize_values(&invalid).is_err());
}

#[test]
fn normalizes_only_exact_bare_mapping_tokens_preserving_comments_and_crlf() {
    let source = "# @@: unchanged\r\nvalues:\r\n  @@: No # default: @@\r\n  yes: Yes\r\ndefault: @@  # unchanged @@\r\n'quoted:key': @@\r\nlabel: prefix@@suffix\r\n";
    let expected = "# @@: unchanged\r\nvalues:\r\n  '@@': No # default: @@\r\n  yes: Yes\r\ndefault: '@@'  # unchanged @@\r\n'quoted:key': '@@'\r\nlabel: prefix@@suffix\r\n";
    assert_eq!(normalize_legacy_yaml(source), expected);
    assert_eq!(normalize_legacy_yaml(expected), expected);
    for unsupported in [
        "@@",
        "value: @@@",
        "value: @@ text",
        "value: [@@]",
        "value: {@@: No}",
    ] {
        assert_eq!(normalize_legacy_yaml(unsupported), unsupported);
    }
}

#[test]
fn quoted_strings_and_multiline_quoted_text_are_untouched() {
    let source = r#"single: 'default: @@'
double: "@@: No"
multiline: 'first line
  @@: No
  default: @@
  final ''quoted'' line'
escaped: "first \"quote
  default: @@
  end"
default: @@
"#;
    let expected =
        source.strip_suffix("default: @@\n").unwrap().to_string() + "default: '@@'\n";
    assert_eq!(normalize_legacy_yaml(source), expected);
    let values = parse_values(source).unwrap();
    assert!(values["multiline"].as_str().unwrap().contains("@@: No"));
    assert!(values["escaped"].as_str().unwrap().contains("default: @@"));
}

#[test]
fn literal_and_folded_blocks_preserve_token_like_text() {
    for indicator in ["|", "|-", "|+", "|2-", ">", ">-", ">2+"] {
        let source = format!(
            "value: {indicator} # header\n  @@: No\n  default: @@\n\n  [[include Box]]\nnext: @@\n"
        );
        let expected =
            source.strip_suffix("next: @@\n").unwrap().to_string() + "next: '@@'\n";
        assert_eq!(
            normalize_legacy_yaml(&source),
            expected,
            "indicator {indicator}"
        );
        let values = parse_values(&source).unwrap();
        assert!(values["value"].as_str().unwrap().contains("@@: No"));
    }
}

#[test]
fn extension_sequence_and_root_blocks_are_not_rewritten() {
    let source = "fields: {}\nnotes:\n  - |\n    @@: No\n    default: @@\ndefault: @@\n";
    let expected =
        "fields: {}\nnotes:\n  - |\n    @@: No\n    default: @@\ndefault: '@@'\n";
    assert_eq!(normalize_legacy_yaml(source), expected);
    let schema = parse_schema(source).unwrap();
    assert_eq!(
        schema.properties["notes"][0],
        Value::String("@@: No\ndefault: @@\n".into())
    );
    let root_block = "|\n  default: @@\n  @@: No\n";
    assert_eq!(normalize_legacy_yaml(root_block), root_block);
}

#[test]
fn compact_flow_quoted_text_preserves_braces_and_multiline_tokens() {
    let source =
        "fields: {}\nextra: {\"text\":\"brace }\n  default: @@\n  end\"}\ndefault: @@\n";
    let expected = "fields: {}\nextra: {\"text\":\"brace }\n  default: @@\n  end\"}\ndefault: '@@'\n";
    assert_eq!(normalize_legacy_yaml(source), expected);
    let schema = parse_schema(source).unwrap();
    assert_eq!(
        schema.properties["extra"]["text"],
        Value::String("brace } default: @@ end".into())
    );
}

#[test]
fn plain_apostrophes_and_flow_quotes_do_not_hide_later_bare_tokens() {
    let source = "after: The character's model\nmetadata: {text: 'default: @@', quoted: \"@@: No\"}\ndefault: @@\n";
    let expected = "after: The character's model\nmetadata: {text: 'default: @@', quoted: \"@@: No\"}\ndefault: '@@'\n";
    assert_eq!(normalize_legacy_yaml(source), expected);
}
