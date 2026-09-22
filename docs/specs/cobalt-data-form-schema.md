# Wikidot data-form schema compatibility

`deepwell/wikidot-forms/` is a standalone pure Rust library for transporting Cobalt's form schemas and stored scalar values across future rendering/editor boundaries. It does not implement a working editor or website.

## What it must do

- [x] Separate one literal `[[form]]...[[/form]]` definition while preserving every byte outside the markers; ordinary templates remain unchanged and ambiguous delimiters fail.
- [x] Preserve field order and select code/label order for `static`, `text`, `select`, and `wiki` definitions.
- [x] Preserve scalar types, labels, dimensions, defaults, unknown properties, and property capitalization, including distinct `Hint` and `hint` keys.
- [x] Serialize the schema for an editor without dropping option codes or unknown properties.
- [x] Normalize only bare `@@` block-mapping keys and whole scalar values before parsing, without altering quoted strings, comments, block text, or line endings.
- [x] Parse and serialize ordered stored field mappings without dropping unknown fields or changing decoded scalar types/content, including Unicode and multiline wiki markup.
- [x] Reject malformed YAML, duplicate mapping keys, unsupported types/shapes, and non-scalar stored values explicitly.

## Standalone form view payload

`extract_form_view(template, page_yaml)` returns `Result<Option<FormView>, FormError>`. No form block returns `None` without parsing page source; malformed delimiters, schema, or values fail explicitly. Populated page records are entire YAML scalar mappings, not delimited regions within unrelated wikitext.

The Serde payload contains `schema` and `values`. Schema fields/options remain ordered arrays; unknown properties and values retain decoded YAML types. Stored field keys must be nonempty strings. JSON null, boolean, number, and string values are not stringified. No new dependencies are required.

This payload remains standalone: no page-view endpoint, rendering, queries, or frontend is wired. `tests/form_view.rs` asserts the serialized JSON contract and decoded value round-trip, legacy `@@`, ordinary templates, and explicit errors.

## How it works

- [Replica proof boundaries](../wiki/systems/cobalt-replica-status.md): pure-library proof only; backend/editor/rendering integration remains open.
- [Public API and representation](../../deepwell/wikidot-forms/src/lib.rs): `split_template`, `normalize_legacy_yaml`, `parse_schema`, `parse_values`, and `serialize_values`.
- [Schema representation](../../deepwell/wikidot-forms/src/schema.rs): fields/options are vectors; remaining properties are ordered YAML mappings. Known scalar properties are validated as scalars, not coerced. Unknown properties are retained as YAML data, not interpreted as editor behavior.
- [Legacy normalization](../../deepwell/wikidot-forms/src/legacy.rs): one preprocessing pass protects quoted and block text. It does not retry a failed parse with different semantics.

Dependencies: Serde supplies the transport serialization contract; maintained `serde_yaml_ng` 0.10 supplies YAML parsing, ordered mappings, and value serialization. `serde_json` is a test-only dependency proving the editor-facing serialization. No I/O/runtime dependency is required.

## Implementation inventory

- `deepwell/wikidot-forms/Cargo.toml`, `Cargo.lock` — independent dependency/build boundary, separate from the DB/network backend.
- `deepwell/wikidot-forms/src/lib.rs` — public API exports and scalar round-trip contract.
- `deepwell/wikidot-forms/src/template.rs` — lossless surrounding-template separation.
- `deepwell/wikidot-forms/src/schema.rs` — ordered schema, supported types, and shape validation.
- `deepwell/wikidot-forms/src/values.rs` — scalar mapping parsing/serialization.
- `deepwell/wikidot-forms/src/legacy.rs` — narrowly scoped `@@` lexical compatibility.
- `deepwell/wikidot-forms/src/error.rs` — explicit delimiter/YAML/shape errors.

## Tests asserting this spec

`deepwell/wikidot-forms/tests/compatibility.rs` uses only synthetic inputs. Run `cargo test --manifest-path deepwell/wikidot-forms/Cargo.toml` with a target directory outside the checkout.

## Known gaps (current cycle)

- [ ] The legacy NPC definition's apparent `orc: Orc:` syntax remains an error, not an automatic repair. The source inventory contains no saved NPC records. No real template or private record is included in fixtures.
- [ ] Main integration must wire this crate into Deepwell rendering and editor APIs; this library alone changes no runtime behavior.
- [ ] Independent verification, readability, and broader checks belong to the integration owner.

## Out of scope

HTML rendering, radio/dropdown choice, permissions, DB/network access, form-value validation against a schema, and migration are owned by the caller. Unknown property semantics are not invented. YAML serialization preserves decoded scalar values/order, not original comments, quote style, or lexical numeric formatting. Template splitting recognizes literal delimiters, not surrounding wiki escaping. Bare `@@` in flow collections and unrelated invalid YAML are not repaired.
