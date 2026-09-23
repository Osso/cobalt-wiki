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
- [x] Save one site/page-target draft shared by authorized editors for either current-page edit or creation. This target/ownership model is a documented source-based inference, not verified current-server behavior.
- [x] Store draft `title` and `wikitext` exactly. Derive complete typed form values, including unknown keys, from stored source on read. Preserve unchanged restored source bytes; merge complete values when fields are edited.
- [x] On a saved draft, offer Edit Original or Edit Draft before normal save; original publishing discards the draft. Cancel offers leave or delete.
- [x] Authorize draft access against an existing origin page; creation-only permission must not expose a draft whose private source page was moved or deleted. Preserve inaccessible orphan rows without automatic relocation.

## How it works

- [Form schema contract](cobalt-data-form-schema.md)
- The raw wikitext formatting toolbar uses 22×22 icons and the three-row control order measured from the authenticated Wikidot editor (`/tmp/claude/wikidot-editor-reference/toolbar-dom.json`, `toolbar-styles.json`, September 23, 2026). Its local `framerail/static/cobalt-editor/icons1.png` is the source sprite from `https://d3g0gp89917ko0.cloudfront.net/v--05014b438f4f/common--theme/base/images/editor/icons1.png` (SHA-256 `d4b09792783e799e3dfd3acc0590f59783a9875909b5baf88f43f5d384d6acd3`); heading levels 2–6 and directional clear-float controls are nested. The mounted raw editors use the implemented 36 source-backed transformations; source wizards are omitted.
- Preview requests are form-encoded as required by SvelteKit actions.

### Save Draft contract and provenance

The user selected `sameasWikidot`; the contract therefore follows captured hosted-editor behavior where evidence exists and source-based inference where it does not. The public `gabrys/wikidot` snapshot 0.90 (July 2009; HEAD September 2009) has no page-draft storage or actions. Captured hosted JavaScript shows title/source saving through `synchronize`, **Edit Original**/**Edit Draft** restoration, Cancel **Leave**/**Delete**, and publishing the original discarding the draft. It does not establish current server ownership, visibility, or lifecycle behavior.

The implemented contract infers one shared draft per site/page target, gated by **View** and **Edit** for an existing page, or **Create** for a missing target. It is not a per-actor draft contract. The hosted read-only check returned `status="ok"`, `draftExists=true`, and `publishedPagePresent=false`; it confirms an unpublished draft exists but captured no source Save/Cancel/Delete action or mutation.

Local proof is not hosted-server proof. `/tmp/claude/cobalt-page-draft-browser-fifth.log` passed 1/1: a missing target saves title/source without a published page or search hit; leave/reopen and **Edit Draft** restore it; publishing after **Edit Draft** or **Edit Original** clears it; **Cancel → Delete** explicitly deletes it; and a restored structured draft, then edit, preserves numeric and unknown values. Anonymous get/save/delete RPCs and SvelteKit actions were denied. Native draft tests passed 13 at `835eaf7`; `9d20256` separately proved the delete-response RED (`null`) to GREEN (`{ deleted: true }`) and the local backend alone was deployed.

Draft rows store inline `wikitext` and `title`; `form_values` is derived on read, not a separate stored map. Restoration returns unchanged raw bytes; changed source uses a valid JSON/YAML merge. The security boundary is the origin page: creation-only access cannot recover a draft from a private page after move/deletion. Such orphan rows remain inaccessible and are not automatically relocated.

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
- `deepwell/tests/page_draft.rs`: native draft authorization, save/restore/delete and typed-value coverage; 13 passed at `835eaf7`. `9d20256` adds the confirmed delete response assertion.
- `/tmp/claude/cobalt-page-draft-browser-fifth.log`: authenticated local browser lifecycle 1/1, including unpublished-target save/reopen/restore/publish/delete, structured typed-value preservation, and anonymous denial.
- `/tmp/claude/cobalt-preview-911748e-native.log`: native 6/6 proof of authorized missing/existing raw previews, structured preview merge and stale/invalid rejection, trusted request identity/access before submitted validation, and no revisions from invalid/conflicting preview input.
- `/tmp/claude/cobalt-page-preview-toolbar-browser.log`: browser 1/1 proof that raw create and existing previews use a real bold-toolbar caret/focus interaction and render raw heading/bold HTML; structured Name preview renders; no publication/revision/source/form changes occur; anonymous preview is denied.
- Run with Node's built-in test runner: `node --test tests/form-editor.test.ts tests/form-fields.test.mjs` from Framerail. Uses the existing `jiti` dependency for extensionless TypeScript imports and existing Svelte dependencies; no full application build/server.

## Known gaps (current cycle)

- [ ] Hosted-server ownership, visibility, and lifecycle parity remain unproven. The shared target/ownership model is authorized source-based inference; the captured source check has no Save/Cancel/Delete action.
- [ ] Six source wizards (table, code, URL, page, image, equation), quick reference/snippets, and watcher-checkbox semantics are unimplemented.
- [ ] Toolbar transformation behavior is source-backed and the bold path has browser proof, but full visual/editor parity is not established.
- The retained `/tmp/claude/cobalt-forms-browser-fourth.log` covers text/wiki/radio/select edits and typed/unknown-value preservation. Restored-draft merge tests and the draft browser scenario add complete-value preservation coverage; they do not establish every field type's full source parity.

## Out of scope

Source-site writes, production deployment without approval, source ACL parity, and unrelated visual redesign are outside this local editor implementation.
