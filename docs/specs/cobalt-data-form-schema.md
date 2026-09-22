# Wikidot data-form schema compatibility

`deepwell/wikidot-forms/` is a standalone pure Rust library for Cobalt form schemas and stored scalar values. Deepwell now consumes it for optional page-view payloads and authorized whole-record updates; it still does not provide a working frontend form editor or complete form workflow.

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

`GetPageViewOutput::Found` now includes optional `form` data from this payload. Deepwell loads the same-site category's `_template` latest raw revision and parses the entire current page source as YAML. Default-category pages use `_template`; template pages themselves remain ordinary source. Missing, invisible, or non-form templates return `None`; malformed definitions or values fail explicitly. Existing wikitext and compiled HTML are unchanged.

Page visibility supplies the actual page ID to the existing attribution-based `page-author` role. Template visibility independently supplies the template's page ID with the same viewer/site and `Page` / `View` category permission; owning a data page does not grant access to another author's template. Page-context decisions neither read nor write the category-only permission cache, whose keys omit page identity. Context-free category checks retain their existing cache behavior. Member roles and category policies are unchanged; missing pages retain the existing missing-page response without source or form disclosure. `tests/page_view_privacy.rs` exercises creator, other-member, anonymous, cross-page/template ordering, member-category access, and missing-page behavior against the native integration services. At `6ab5129`, `cargo test --offline --locked --test page_view_privacy -- --nocapture` passed its DB regression (1/1, no compiler warnings). The preceding context-only implementation failed because viewing an owned page exposed another author's template through the category cache. This proof does not cover production ACL import, attachment authorization, or deployment. That slice adds no frontend, saving, rendering, or query behavior. The pure `FormView` is serialized once to a JSON value at the backend response boundary, preserving its payload contract while satisfying JSON-RPC's `Clone` response requirement without changing the standalone types. Serialization failures are explicit.

`tests/form_view.rs` asserts the standalone JSON contract. Backend `services/view/form.rs` tests category/default template selection, template self-exclusion, absent/non-form templates, full-record JSON values, and explicit malformed-input errors. Four helper tests passed through an offline isolated harness importing that exact backend module. This does not prove database lookup, permission execution, or endpoint integration. Initial backend offline resolution lacked the `arraystring` index entry. Resolution then passed using existing local Nix-vendored dependencies plus Cargo-vendored standalone YAML dependencies, without network access. Lock additions retain existing backend versions and YAML package checksums from the standalone lockfile. At `829b88e`, the offline locked backend binary test build passed in 8m42s, compiling the backend library and RPC registration. The binary-target filter ran zero tests (the helper tests live in the library); the four helper tests were proven separately by the isolated harness. Database lookup, permission execution, and live endpoint behavior remain unproven. No full `cargo check`, broad suite, or network operation was run.

## Applying field updates

`apply_field_updates(&FormSchema, &Mapping, &Mapping)` returns serialized YAML for the entire original record through `serialize_values`. Updates must name schema-defined fields and contain scalar values. Unknown original fields and untouched scalar types/order survive. Static fields may be submitted unchanged but cannot be changed or added. Changed select values must equal a declared option code using YAML scalar equality; an unrecognized existing code survives when omitted or submitted unchanged. Missing values are distinct from explicit nulls.

No required/default rules, stringification, or empty-string-to-null coercion are introduced. Malformed update keys, nested/tagged updates, static changes, and new invalid select codes return explicit `FormError` messages. `tests/updates.rs` covers whole-map edits, retained unknown fields and scalar types, static restrictions, typed select codes, and legacy `@@` round-trips. Endpoint wiring is described below; frontend wiring remains open.

## Backend structured edits

`page_edit` accepts optional `form_updates`, a mapping of field names to JSON scalars, in an endpoint-only wrapper around the unchanged `EditPage` service request. Raw `wikitext` and `form_updates` are mutually exclusive, including empty strings/maps. Omitting `form_updates` preserves the original raw/metadata-only edit mode. A null mapping is invalid; a null field value is an explicit scalar update.

The existing `Action::Edit` check runs before mode validation or form/source reads. Structured edits load the same-site page, reject a stale `last_revision_id` before loading source, and check the fetched latest revision again before reading its text. The same-site category template must contain a valid form; missing, invisible, ordinary, malformed, and self-template cases fail explicitly. Template lookup reuses page-view visibility with the request-context viewer, not the submitted attribution `user_id`.

Updates apply to the entire stored YAML mapping using `apply_field_updates`. Only the service request's `wikitext` is populated; caller revision, attribution user, comments, IP, and other edits remain unchanged. `PageService::edit` retains its existing filter and optimistic revision check. No frontend or new ACL semantics are included.

- [x] Wire scalar updates preserve unknown stored values and decoded types; invalid fields/static changes/select codes/nested values fail.
- [x] Raw and form modes conflict; absent updates preserve the raw request.
- [x] The reused revision guard rejects a different latest revision without rewriting the caller's revision.
- [x] The native DB harness proves authorization ordering, template self-exclusion, scalar update persistence, and stale-edit non-mutation for the three endpoint cases below.

Six filtered backend library tests passed after behavioral RED. `deepwell/tests/page_form_edit.rs` contains three endpoint tests: scalar update followed by an intervening raw edit and stale submission preserves the newer source; invalid/conflicting/template updates create no revision; anonymous request-context denial occurs before malformed form validation and ignores an administrator attribution in the body. On September 22, 2026, an isolated local PostgreSQL 17, Valkey, and Silo environment ran all three tests successfully after migrations and the stock development seeder; protected log: `/home/osso/.local/share/cobalt-wiki/integration/page-form-edit-tests.log`. Four view-helper tests also passed; protected log: `/home/osso/.local/share/cobalt-wiki/integration/form-view-tests.log`. This proves those seeded local endpoint paths, not production data, concurrent interleavings beyond the stale-revision case, deployment, or source ACL parity.

## How it works

- [Replica proof boundaries](../wiki/systems/cobalt-replica-status.md): pure-library and bounded backend payload/edit proof; frontend, rendering, DB-backed behavior, and workflow integration remain open.
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
- `deepwell/src/endpoints/page.rs`, `page/form_edit.rs` — authorized wire modes, latest-source loading and structured edit preparation.
- `deepwell/src/services/view/{form,service}.rs` — shared template selection, extraction and visibility.
- `deepwell/src/services/page/service.rs` — reused revision guard; existing final edit check unchanged.

## Tests asserting this spec

`deepwell/wikidot-forms/tests/compatibility.rs` uses only synthetic inputs. Run `cargo test --manifest-path deepwell/wikidot-forms/Cargo.toml` with a target directory outside the checkout.

## Known gaps (current cycle)

- [ ] The legacy NPC definition's apparent `orc: Orc:` syntax remains an error, not an automatic repair. The source inventory contains no saved NPC records. No real template or private record is included in fixtures.
- [ ] Frontend controls have seven SSR/model tests, but hydrated interaction and save/reload roundtrip remain unproven. Backend payload/edit wiring and local endpoint cases are not form-workflow parity.
- [ ] Private attachment authorization is separate and missing: current WWS attachment routes do not enforce page-view authorization. Do not expose private attachments.
- [ ] Independent verification, readability, and broader checks belong to the integration owner.

## Out of scope

HTML rendering, radio/dropdown choice, permissions, DB/network access, validation beyond the field-update rules above, and migration are owned by the caller. Unknown property semantics are not invented. YAML serialization preserves decoded scalar values/order, not original comments, quote style, or lexical numeric formatting. Template splitting recognizes literal delimiters, not surrounding wiki escaping. Bare `@@` in flow collections and unrelated invalid YAML are not repaired.
