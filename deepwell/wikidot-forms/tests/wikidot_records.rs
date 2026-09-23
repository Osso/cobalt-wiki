use wikidot_forms::{
    Mapping, Value, new_record, parse_schema, parse_values, serialize_values,
};

// Excerpt of the archived character:_template form, including a legacy bare `@@` default.
const CHARACTER_FORM: &str = "fields:
  header-player:
    type: static
    label: Player
    value: Your OOC player name/handle.
  player:
    label:
    type: text
  name:
    label: Name
    type: text
  aka:
    label: AKA
    type: text
    default: '@@'
  portrait:
    label:
    type: text
    default: @@
  sex:
    label: Body Type
    type: select
    values:
      female: Female
      male: Male
";

#[test]
fn new_pages_save_every_field_as_posted_form_text() {
    let schema = parse_schema(CHARACTER_FORM).unwrap();
    let submitted: Mapping =
        serde_yaml_ng::from_str("name: Isabeau Clark\nsex: female\n").unwrap();
    assert_eq!(
        new_record(&schema, &submitted).unwrap(),
        "player: ''\nname: 'Isabeau Clark'\naka: '@@'\nportrait: '@@'\nsex: female",
    );
}

#[test]
fn new_pages_reject_unknown_fields_and_undeclared_select_codes() {
    let schema = parse_schema(CHARACTER_FORM).unwrap();
    for submitted in ["nickname: Iz\n", "sex: other\n", "header-player: Changed\n"] {
        let submitted: Mapping = serde_yaml_ng::from_str(submitted).unwrap();
        assert!(new_record(&schema, &submitted).is_err(), "{submitted:?}");
    }
}

/// Every archived form record re-serializes to Wikidot's exact bytes.
/// `COBALT_ARCHIVE_SOURCE` names the backup's `source` directory.
#[test]
#[ignore = "needs the Cobalt archive; set COBALT_ARCHIVE_SOURCE and run with --ignored"]
fn archived_form_records_round_trip_byte_for_byte() {
    let directory = std::env::var("COBALT_ARCHIVE_SOURCE").unwrap();
    let categories = [
        "character",
        "player",
        "bgc",
        "npc",
        "writing",
        "arc",
        "chain",
        "application",
    ];
    let (mut checked, mut different) = (0, Vec::new());
    for entry in std::fs::read_dir(directory).unwrap() {
        let path = entry.unwrap().path();
        let name = path.file_name().unwrap().to_string_lossy().into_owned();
        let category = name.split('_').next().unwrap();
        if !categories.contains(&category) || name.ends_with("__template.txt") {
            continue;
        }
        let source = std::fs::read_to_string(&path).unwrap();
        let Ok(values) = parse_values(&source) else {
            continue;
        };
        checked += 1;
        if serialize_values(&values).unwrap() != source {
            different.push(name);
        }
    }
    assert!(checked > 5000, "only {checked} records found");
    assert!(
        different.is_empty(),
        "{} of {checked} differ: {:?}",
        different.len(),
        &different[..different.len().min(10)]
    );
}
