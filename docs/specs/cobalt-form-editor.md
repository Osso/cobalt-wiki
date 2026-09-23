# Source-defined form editor

Framerail edits the optional backend `Found.form` payload using ordered source-defined fields. Schema/value parsing and preservation remain backend responsibilities; see [data forms](cobalt-data-form-schema.md).

## What it must do

- [x] Preserve unchanged scalar types, including numeric text values and nulls; defaults display without generating updates.
- [x] Submit only changed editable known fields; omit static and unknown values from updates. Select codes retain scalar types; text/wiki edits are strings.
- [x] Construct mutually exclusive `form_updates` and `wikitext` wire payloads, including empty update maps.
- [x] Render ordered static/text/select/wiki fields with visible labels, source hints, defaults and supported numeric dimensions. A select with 2–4 choices uses labeled radio buttons; larger sets remain dropdowns. Radio choices retain numeric/string/boolean/null codes, including distinct numeric `1` and string `"1"` values. Missing or unrecognized stored values are not silently replaced by the first choice. Static content remains escaped readonly text.
- [x] Use form controls whenever a form exists; retain the raw editor only for nonform/template pages.
- [x] Preserve existing title, alternate title, tags, comments, revision and authentication handling through the edit action.
- [x] Preview raw or structured draft content through the SvelteKit action without publishing, creating a revision, or changing stored page/source/form data. The trusted request identity and access check precede submitted form validation; anonymous and spoofed requests are denied.

## How it works

- [Form schema contract](cobalt-data-form-schema.md)
- The raw wikitext formatting toolbar uses 22×22 icons and the three-row control order measured from the authenticated Wikidot editor (`/tmp/claude/wikidot-editor-reference/toolbar-dom.json`, `toolbar-styles.json`, September 23, 2026). Its local `framerail/static/cobalt-editor/icons1.png` is the source sprite from `https://d3g0gp89917ko0.cloudfront.net/v--05014b438f4f/common--theme/base/images/editor/icons1.png` (SHA-256 `d4b09792783e799e3dfd3acc0590f59783a9875909b5baf88f43f5d384d6acd3`); heading levels 2–6 and directional clear-float controls are nested. The mounted raw editors use the implemented 36 source-backed transformations; source wizards are omitted.
- Preview requests are form-encoded as required by SvelteKit actions. Static source script and handler-definition review identifies source save through `synchronize`, a restore offer of **Edit Original**/**Edit Draft**, and cancel choices to leave or delete a draft; no source save/action was executed for this inventory.

## Implementation inventory

- `framerail/src/lib/form-editor.ts`: scalar transport types, draft/diff model and exclusive content payload.
- `framerail/src/lib/component/DataFormFields.svelte`: source-defined controls and labels.
- `framerail/src/routes/[slug]/[...extra]/EditorPane.svelte`: selects form or raw editor, mounts the toolbar for raw create/edit, and submits changed fields.
- `framerail/src/lib/component/EditorPreview.svelte`, `framerail/src/lib/server/load/page-preview.ts`, `deepwell/src/endpoints/page_preview.rs`: form-encoded preview action and authorized no-write rendering.
- `framerail/src/lib/server/deepwell/views.ts`: optional backend form response type.
- `framerail/src/lib/server/load/page.ts`: forwards form data and validates exclusive edit inputs.
- `framerail/src/lib/server/deepwell/page.ts`: sends the selected content variant to Deepwell.

## Tests asserting this spec

- `framerail/tests/form-editor.test.ts`: concrete draft changes, scalar codes, readonly/unknown exclusions and wire payloads.
- `framerail/tests/form-fields.test.mjs`: bounded Svelte server-rendered radio/dropdown controls, typed selection, unknown/unselected values and static text; not browser interaction proof.
- `/tmp/claude/cobalt-preview-911748e-native.log`: native 6/6 proof of authorized missing/existing raw previews, structured preview merge and stale/invalid rejection, trusted request identity/access before submitted validation, and no revisions from invalid/conflicting preview input.
- `/tmp/claude/cobalt-page-preview-toolbar-browser.log`: browser 1/1 proof that raw create and existing previews use a real bold-toolbar caret/focus interaction and render raw heading/bold HTML; structured Name preview renders; no publication/revision/source/form changes occur; anonymous preview is denied.
- Run with Node's built-in test runner: `node --test tests/form-editor.test.ts tests/form-fields.test.mjs` from Framerail. Uses the existing `jiti` dependency for extensionless TypeScript imports and existing Svelte dependencies; no full application build/server.

## Known gaps (current cycle)

- [ ] Save Draft server ownership/access semantics are unknown. Source review alone shows `synchronize`, restore offers for **Edit Original**/**Edit Draft**, and cancel leave/delete choices.
- [ ] Six source wizards (table, code, URL, page, image, equation), quick reference/snippets, and watcher-checkbox semantics are unimplemented.
- [ ] Toolbar transformation behavior is source-backed and the bold path has browser proof, but full visual/editor parity is not established.
- [ ] Backend unknown-value preservation is relied upon, not reimplemented by frontend tests.
- [ ] Browser interaction with radio choices and typed save payload remains unproven; SSR/model checks do not establish hydrated behavior.

## Out of scope

Backend changes beyond preview, rendering saved wiki content, source save/action behavior, migration, deployment, new validation requirements, and visual redesign are outside this frontend slice.
