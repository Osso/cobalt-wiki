# Wikidot data-form schema compatibility

`deepwell/wikidot-forms/` is a standalone pure Rust library for Cobalt form schemas and stored scalar values. Deepwell consumes it for optional page-view payloads and authorized whole-record updates; Framerail has a bounded hydrated save/reload flow. Source rich-editor parity remains unfinished.

## What it must do

- [x] Separate one literal `[[form]]...[[/form]]` definition while preserving every byte outside the markers; ordinary templates remain unchanged and ambiguous delimiters fail.
- [x] Preserve field order and select code/label order for `static`, `text`, `select`, and `wiki` definitions.
- [x] Preserve scalar types, labels, dimensions, defaults, unknown properties, and property capitalization, including distinct `Hint` and `hint` keys.
- [x] Serialize the schema for an editor without dropping option codes or unknown properties.
- [x] Normalize bare `@@` block-mapping tokens and a narrow legacy plain block-mapping value: it starts alphanumeric, is outside flow syntax, contains no earlier colon, and ends in a colon (including `orc: Orc:`). Preserve the literal label, quoted strings, comments, block text, and line endings. Do not repair unrelated malformed YAML or flow syntax.
- [x] Parse and serialize ordered stored field mappings without dropping unknown fields or changing decoded scalar types/content, including Unicode and multiline wiki markup.
- [x] Reject malformed YAML, duplicate mapping keys, unsupported types/shapes, and non-scalar stored values explicitly.
- [x] Serialize records exactly as Wikidot saves them: `sfYaml::dump($values, 999)` (`~/Repos/wikidot/php/class/Wikidot/Yaml.php`) with the Symfony YAML escaper (double quotes for control characters, NBSP, U+0085/2028/2029; single quotes for spaces/indicators; digit, numeric, timestamp and true/false/null strings quoted), and no trailing newline. All 5,969 archived form records re-serialize byte for byte (`wikidot-forms/tests/wikidot_records.rs`, ignored test run with `COBALT_ARCHIVE_SOURCE`).

## Missing-page creation

`page_create_permission` derives `can_create` only from trusted request site, actor, and slug context. `page_create` requires the same context and persists submitted tags on its first revision; body `user_id`, site ID, or slug cannot substitute for it. `deepwell/tests/page_create_permission.rs` passed its two native DB cases: anonymous/forged actor, site, or slug requests are denied without creating a page; matching trusted context creates a page whose content, tags, and revision attribution round-trip.

The missing-page frontend asks for that permission through the server request context, shows the create editor only when it is granted, and initializes the existing title, alt-title, source, tags, layout, and comment defaults. This is not source rich-editor parity.

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

Updates apply to the entire stored YAML mapping using `apply_field_updates`. Only the service request's `wikitext` is populated; caller revision, attribution user, comments, IP, and other edits remain unchanged. `PageService::edit` retains its existing filter and optimistic revision check. No source rich-editor parity or new ACL semantics are included.

- [x] Wire scalar updates preserve unknown stored values and decoded types; invalid fields/static changes/select codes/nested values fail.
- [x] Raw and form modes conflict; absent updates preserve the raw request.
- [x] The reused revision guard rejects a different latest revision without rewriting the caller's revision.
- [x] The native DB harness proves authorization ordering, template self-exclusion, scalar update persistence, and stale-edit non-mutation for the three endpoint cases below.

Six filtered backend library tests passed after behavioral RED. `deepwell/tests/page_form_edit.rs` contains three endpoint tests: scalar update followed by an intervening raw edit and stale submission preserves the newer source; invalid/conflicting/template updates create no revision; anonymous request-context denial occurs before malformed form validation and ignores an administrator attribution in the body. On September 22, 2026, an isolated local PostgreSQL 17, Valkey, and Silo environment ran all three tests successfully after migrations and the stock development seeder; protected log: `/home/osso/.local/share/cobalt-wiki/integration/page-form-edit-tests.log`. Four view-helper tests also passed; protected log: `/home/osso/.local/share/cobalt-wiki/integration/form-view-tests.log`. This proves those seeded local endpoint paths, not production data, concurrent interleavings beyond the stale-revision case, deployment, or source ACL parity.

## Hydrated editor acceptance

The local browser scenario at committed `4f407d0` passed `1/1` against runtime `bc6d5e2` (`/tmp/claude/cobalt-forms-browser-fourth.log`). A real UI login edited text, wiki, radio, and select controls; save/reload retained their typed values, three untouched schema values, and an unknown `true` field. An anonymous browser saw no editor and its direct action submission returned SvelteKit's failure protocol without creating a revision. Cookie percent-decoding and the action-failure protocol are part of that harness result, not application behavior guarantees.

This is a bounded form workflow proof. Formatting toolbar remains incomplete. Save Draft has separate local browser/native proof in the [form-editor contract](cobalt-form-editor.md); that does not establish hosted-server draft ownership or lifecycle parity.

## Source preview

- [x] Authorized create and edit forms can request a read-only preview of submitted source without persisting a page or revision.
- [x] Preview uses the form's trusted request context; submitted site, actor, and page identity do not authorize it.

`d9f5f79` added the backend preview endpoint and six real-DB preview/form cases. They passed 6/6 and prove no persistence for that bounded path. `ffb3cc4` and `85c4e09` add the frontend preview action/component and its tests; `fd02305` connects it to create and edit forms. Runtime and browser acceptance remain unproven. Draft semantics and server ownership are unknown. The source-backed formatting toolbar is in progress, not complete.

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
- `deepwell/src/endpoints/page_preview.rs` — read-only authorized source-preview endpoint.
- `deepwell/src/services/view/{form,service}.rs` — shared template selection, extraction and visibility.
- `framerail/src/lib/component/EditorPreview.svelte`, `src/lib/server/{deepwell,load}/page-preview.ts` — bounded preview UI and server-side request/load path.
- `framerail/src/routes/[slug]/[...extra]/{+page.server.ts,EditorPane.svelte}` — create/edit form integration.
- `deepwell/src/services/page/service.rs` — reused revision guard; existing final edit check unchanged.

## Tests asserting this spec

`deepwell/wikidot-forms/tests/compatibility.rs` uses only synthetic inputs. Run `cargo test --manifest-path deepwell/wikidot-forms/Cargo.toml` with a target directory outside the checkout.

`deepwell/tests/page_preview.rs` contains the six real-DB preview/form cases proven at `d9f5f79`. Frontend and authenticated browser preview acceptance, including its boundaries, are the [form-editor SSOT](cobalt-form-editor.md#tests-asserting-this-spec).

## Historical NPC parser evidence

The Wikidot repository's `git ls-tree` recovers the exact historical Symfony YAML submodule pin: `f3abfaa5228e7e81954e38b0757de2ad19875bc8`. The public `fabpot/yaml` checkout at `/tmp/claude/cobalt-legacy-sfyaml` was pinned to that commit and evaluated with PHP 8.5's archived `sfYamlParser`; its wrapper input included the historical `---` prefix. The full archived NPC schema parsed successfully (`/tmp/claude/cobalt-npc-pinned-sfyaml.log`, exit 0): 22 fields (7 static, 9 text, 4 select, 2 wiki), and the `race` option parsed with code `orc` and literal label `Orc:`.

This supersedes the former claim that historical parser behavior was unavailable or unproven. It establishes acceptance by that pinned legacy parser only—not behavior of the current hosted implementation. Approved standalone compatibility accepts `orc: Orc:` as code `orc`, literal label `Orc:`. Its normalizer is deliberately narrow: only a non-flow block-mapping value that starts alphanumeric, has no earlier colon, and ends in a colon; quoted text, comments, block text, line endings, and malformed multi-colon values retain their boundaries. `deepwell/wikidot-forms/tests/compatibility.rs` passed 19/19 at `4f687c9` (`/tmp/claude/cobalt-npc-compatibility.log`). Independent parser verification 526 reused those 19 cases; the other standalone cases passed 15 with 1 ignored, and offline formatting/checking passed. Changed Rust readability stayed below thresholds. Existing unrelated observations remain: `advance_token` has cyclomatic complexity 21 and `wikidot_records.rs` emits an unused `Value` warning.

The pre-deploy NPC missing-page browser run failed at `page_create_permission` with RPC 4000 (`/tmp/claude/cobalt-npc-form-99bfb2f-schema-red.log`). After root `./deploy.sh` exited 0 (`/tmp/claude/cobalt-npc-local-deploy-4f687c9.log`) and local unit PID 2896759 was active, the current deployed build passed the bounded NPC browser case: all 22 fields (7 static, 9 text, 4 select, 2 wiki), the exact `Orc:` race label/default/options, Preview, reload, and no-write checks (`framerail/tests/local/category-forms.mjs`; commit `99bfb2f`). This is local deployed browser evidence, not hosted behavior. The source inventory still contains no saved NPC records and no archive rewrite is authorized.

## Known gaps (current cycle)

- [ ] Formatting toolbar and source rich-editor parity are incomplete. Preview acceptance is documented in the [form-editor SSOT](cobalt-form-editor.md#tests-asserting-this-spec); it does not establish toolbar or source-parity behavior.
- [ ] Draft ownership and hosted-server lifecycle semantics remain unproven; the local shared-target behavior is documented authorized inference in the [form-editor contract](cobalt-form-editor.md).
- [ ] Independent or broader browser coverage remains open; the single local authenticated form roundtrip does not establish site-wide workflow parity.
- [ ] Private attachment authorization is separate and missing: current WWS attachment routes do not enforce page-view authorization. Do not expose private attachments.
- [ ] Independent verification, readability, and broader checks belong to the integration owner.

## Out of scope

HTML rendering, radio/dropdown choice, permissions, DB/network access, validation beyond the field-update rules above, and migration are owned by the caller. Unknown property semantics are not invented. YAML serialization preserves decoded scalar values/order, not original comments, quote style, or lexical numeric formatting. Template splitting recognizes literal delimiters, not surrounding wiki escaping. Bare `@@` in flow collections and unrelated invalid YAML are not repaired.
