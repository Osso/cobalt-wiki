# Source-defined form editor

Framerail edits the optional backend `Found.form` payload using ordered source-defined fields. Schema/value parsing and preservation remain backend responsibilities; see [data forms](cobalt-data-form-schema.md).

## What it must do

- [x] Preserve unchanged scalar types, including numeric text values and nulls; defaults display without generating updates.
- [x] Submit only changed editable known fields; omit static and unknown values from updates. Select codes retain scalar types; text/wiki edits are strings.
- [x] Construct mutually exclusive `form_updates` and `wikitext` wire payloads, including empty update maps.
- [x] Render ordered static/text/select/wiki fields with visible labels, source hints, defaults and supported numeric dimensions. A select with 2–4 choices uses labeled radio buttons; larger sets remain dropdowns. Radio choices retain numeric/string/boolean/null codes, including distinct numeric `1` and string `"1"` values. Missing or unrecognized stored values are not silently replaced by the first choice. Static content remains escaped readonly text.
- [x] Use form controls whenever a form exists; retain the raw editor only for nonform/template pages.
- [x] Preserve existing title, alternate title, tags, comments, revision and authentication handling through the edit action.

## How it works

- [Form schema contract](cobalt-data-form-schema.md)

## Implementation inventory

- `framerail/src/lib/form-editor.ts`: scalar transport types, draft/diff model and exclusive content payload.
- `framerail/src/lib/component/DataFormFields.svelte`: source-defined controls and labels.
- `framerail/src/routes/[slug]/[...extra]/EditorPane.svelte`: selects form or raw editor and submits changed fields.
- `framerail/src/lib/server/deepwell/views.ts`: optional backend form response type.
- `framerail/src/lib/server/load/page.ts`: forwards form data and validates exclusive edit inputs.
- `framerail/src/lib/server/deepwell/page.ts`: sends the selected content variant to Deepwell.

## Tests asserting this spec

- `framerail/tests/form-editor.test.ts`: concrete draft changes, scalar codes, readonly/unknown exclusions and wire payloads.
- `framerail/tests/form-fields.test.mjs`: bounded Svelte server-rendered radio/dropdown controls, typed selection, unknown/unselected values and static text; not browser interaction proof.
- Run with Node's built-in test runner: `node --test tests/form-editor.test.ts tests/form-fields.test.mjs` from Framerail. Uses the existing `jiti` dependency for extensionless TypeScript imports and existing Svelte dependencies; no full application build/server.

## Known gaps (current cycle)

- [ ] Hydrated browser interaction and save/reload roundtrip require independent runtime verification.
- [ ] Backend unknown-value preservation is relied upon, not reimplemented by frontend tests.
- [ ] Browser interaction with radio choices and typed save payload remains unproven; SSR/model checks do not establish hydrated behavior.

## Out of scope

Backend changes, rendering saved wiki content, migration, deployment, new validation requirements, and visual redesign are outside this frontend slice.
