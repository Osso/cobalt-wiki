# Source-defined form editor

Framerail edits the optional backend `Found.form` payload using ordered source-defined fields. Schema/value parsing and preservation remain backend responsibilities; see [data forms](cobalt-data-form-schema.md).

## What it must do

- [x] Preserve unchanged scalar types, including numeric text values and nulls; defaults display without generating updates.
- [x] Submit only changed editable known fields; omit static and unknown values from updates. Select codes retain scalar types; text/wiki edits are strings.
- [x] Construct mutually exclusive `form_updates` and `wikitext` wire payloads, including empty update maps.
- [x] Render ordered static/text/select/wiki fields with visible labels, source hints, defaults and supported numeric dimensions. A select with 2–4 choices uses labeled radio buttons; larger sets remain dropdowns. Radio choices retain numeric/string/boolean/null codes, including distinct numeric `1` and string `"1"` values. Missing or unrecognized stored values are not silently replaced by the first choice. Static content remains escaped readonly text.
- [x] Use form controls whenever a form exists; retain the raw editor only for nonform/template pages.
- [x] Create pages in a data-form category through the category form (Wikidot's NewPage → form flow). `page_create_permission` returns the category form to users who may create the page; the missing-page editor shows `DataFormFields` (defaults prefilled, title from `/title/<name>`) and save, preview and draft send `form_updates`. The stored source is Wikidot's record: every non-static field in schema order as posted text (submitted, else default, else empty).
- [x] Preserve existing title, alternate title, tags, comments, revision and authentication handling through the edit action.
- [x] Preview raw or structured draft content through the SvelteKit action without publishing, creating a revision, or changing stored page/source/form data. The trusted request identity and access check precede submitted form validation; anonymous and spoofed requests are denied.
- [x] Save one site/page-target draft shared by authorized editors for either current-page edit or creation. This target/ownership model is a documented source-based inference, not verified current-server behavior.
- [x] Store draft `title` and `wikitext` exactly. Derive complete typed form values, including unknown keys, from stored source on read. Preserve unchanged restored source bytes; merge complete values when fields are edited.
- [x] On a saved draft, offer Edit Original or Edit Draft before normal save; original publishing discards the draft. Cancel offers leave or delete.
- [x] Authorize draft access against an existing origin page; creation-only permission must not expose a draft whose private source page was moved or deleted. Preserve inaccessible orphan rows without automatic relocation.
- [x] Provide table, code, URL, page-link, image, and equation-reference wizards for raw editors. Code wraps the captured range; the other five insert at its start without consuming selected text. Equation references insert `Eq.([[eref label]])` or bare `[[eref label]]`, not a wrapper around selected text.
- [x] Use the trusted current page for image attachment lookup, requiring both View and Edit; return active files whose latest revision is an image. A missing target returns no attachment lookup result rather than creating it.
- [x] Use permission-filtered current-content search for page suggestions after two characters and a 0.5-second delay. This is a local lookup contract, not verified source autocomplete parity.

## How it works

- [Form schema contract](cobalt-data-form-schema.md)
- The raw wikitext formatting toolbar uses 22×22 icons and the three-row control order measured from the authenticated Wikidot editor (`/tmp/claude/wikidot-editor-reference/toolbar-dom.json`, `toolbar-styles.json`, September 23, 2026). Its local `framerail/static/cobalt-editor/icons1.png` is the source sprite from `https://d3g0gp89917ko0.cloudfront.net/v--05014b438f4f/common--theme/base/images/editor/icons1.png` (SHA-256 `d4b09792783e799e3dfd3acc0590f59783a9875909b5baf88f43f5d384d6acd3`); heading levels 2–6 and directional clear-float controls are nested. The mounted raw editors use the implemented 36 source-backed transformations.
- Preview requests are form-encoded as required by SvelteKit actions.

### Source wizard evidence and contract

The hosted editor implementation is `/tmp/claude/wikidot-editor-reference/WIKIDOT.editor.pretty.js`; dialog fields come from the older public reference `/home/osso/Repos/wikidot/web/files--common/editor/dialogs.html`. The latter supplies field/default evidence, not current hosted-browser parity. Local implementation and proof are listed below; they do not establish live lookup or visual parity.

- Code wraps the captured selection range, or inserts a selected placeholder at an empty caret. The other five wizards—table, URL, page link, image, and equation reference—insert at the captured selection start without consuming selected text. `erefWizard` inserts a reference to a labelled equation, optionally surrounded by `Eq.(…)`; it does not wrap selected text or create an equation.
- Table defaults to 3 rows and 3 columns, with an optional first-row header. The older dialog permits only two input characters (`maxlength="2"`); integer validation and a 1–99 range are local implementation inference, not source behavior.
- Local implementation supports the source-derived table/code/URL/page-link/image/equation forms. Image accepts URI, attached-file, and Flickr syntax; its position choices map to no position, left, right, center, float-left, and float-right. It intentionally omits the older template's unused extra-CSS field and does not expose size options, which were not captured. Flickr input is normalized locally but no Flickr photo-info check occurs.
- Code offers empty type plus `Cpp`, `CSS`, `PHP`, `HTML`, `diff`, `Java`, misspelled source value `JavaScipt`, `Perl`, `Python`, `Ruby`, `SQL`, and `XML`.
- URL defaults to `http://`, has optional anchor text, and defaults the new-window checkbox off. Page link takes page name plus optional anchor; the local request begins at two characters after a 0.5-second delay and reuses permission-filtered current-content search. It is not exact source title-autocomplete parity.
- Attachment lookup authorizes the trusted site/page/user for both View and Edit, includes only active files whose latest revision is an image, and excludes attachments for a missing target. Browser proof covers an empty attachment list, a missing page with no attached-file choice, and the imported `images` page: selecting an authorized filename loads its thumbnail and rendered Preview image without changing source/revision. The separate attachment test also rejects invalid Flickr input and inserts numeric Flickr syntax without fetching Flickr.
- Equation reference scans the editor's current source for labelled math blocks, displays their source as escaped text, and offers `Eq.(number)` or bare-number output. No network lookup is needed.

### Save Draft contract and provenance

The user selected `sameasWikidot`; the contract therefore follows captured hosted-editor behavior where evidence exists and source-based inference where it does not. The public `gabrys/wikidot` snapshot 0.90 (July 2009; HEAD September 2009) has no page-draft storage or actions. Captured hosted JavaScript shows title/source saving through `synchronize`, **Edit Original**/**Edit Draft** restoration, Cancel **Leave**/**Delete**, and publishing the original discarding the draft. It does not establish current server ownership, visibility, or lifecycle behavior.

The implemented contract infers one shared draft per site/page target, gated by **View** and **Edit** for an existing page, or **Create** for a missing target. It is not a per-actor draft contract. The hosted read-only check returned `status="ok"`, `draftExists=true`, and `publishedPagePresent=false`; it confirms an unpublished draft exists but captured no source Save/Cancel/Delete action or mutation.

Local proof is not hosted-server proof. `/tmp/claude/cobalt-page-draft-browser-fifth.log` passed 1/1: a missing target saves title/source without a published page or search hit; leave/reopen and **Edit Draft** restore it; publishing after **Edit Draft** or **Edit Original** clears it; **Cancel → Delete** explicitly deletes it; and a restored structured draft, then edit, preserves numeric and unknown values. Anonymous get/save/delete RPCs and SvelteKit actions were denied. Native draft tests passed 13 at `835eaf7`; `9d20256` separately proved the delete-response RED (`null`) to GREEN (`{ deleted: true }`) and the local backend alone was deployed.

Draft rows store inline `wikitext` and `title`; `form_values` is derived on read, not a separate stored map. Restoration returns unchanged raw bytes; changed source uses a valid JSON/YAML merge. The security boundary is the origin page: creation-only access cannot recover a draft from a private page after move/deletion. Such orphan rows remain inaccessible and are not automatically relocated.

Independent bounded audit (agent 363) confirmed the browser assertions and one read-only SQL reconciliation: both fixture pages have matching persisted source hashes and no remaining draft rows. At `7445c2a`, Svelte checking reports zero errors/warnings and scoped ESLint, Stylelint, and Prettier pass (`/tmp/claude/cobalt-draft-final-main-*.log`). Independent Rust fmt/check proof is retained from agent 360. Post-SSR hydration passes (`/tmp/claude/cobalt-draft-final-hydration.log`); SSR servers now use isolated temporary caches. Pre-existing route/config/dependency deprecations remain outside these scoped results.

## Implementation inventory

- `framerail/src/lib/form-editor.ts`: scalar transport types, draft/diff model and exclusive content payload.
- `framerail/src/lib/component/DataFormFields.svelte`: source-defined controls and labels.
- `framerail/src/routes/[slug]/[...extra]/EditorPane.svelte`: selects form or raw editor, mounts the toolbar and wizard dialogs for raw create/edit, restores source selection after insert/cancel, and submits changed fields.
- `framerail/src/lib/wikitext-wizards.ts`: pure wizard validation, source generation, selection insertion, equation extraction, and Flickr normalization.
- `framerail/src/lib/component/WikitextWizard.svelte`: six modal wizard forms, delayed page suggestions, attachment selection, URI preview, and escaped equation-source preview.
- `framerail/src/lib/server/load/editor-lookup.ts`, `deepwell/src/endpoints/editor_lookup.rs`: trusted page-suggestion transport and current-page image attachment lookup.
- `framerail/src/lib/component/EditorPreview.svelte`, `framerail/src/lib/server/load/page-preview.ts`, `deepwell/src/endpoints/page_preview.rs`: form-encoded preview action and authorized no-write rendering.
- `framerail/src/lib/server/deepwell/views.ts`: optional backend form response type.
- `framerail/src/lib/server/load/page.ts`: forwards form data and validates exclusive edit inputs.
- `framerail/src/lib/server/deepwell/page.ts`: sends the selected content variant to Deepwell.

## Tests asserting this spec

- `framerail/tests/local/form-create.mjs`: browser 1/1 on an isolated local stack (2026-09-23): NewPage "Create Character Profile" → form editor with prefilled title and no raw source → save stores `player: ''`, `name: '…'`, `'@@'` defaults, `sex: female` in template order, and the live template shows the name.
- `deepwell/tests/page_form_create.rs`: native create/preview/draft through the category form, anonymous denial, invalid select rejection.
- `framerail/tests/form-editor.test.ts`: concrete draft changes, scalar codes, readonly/unknown exclusions and wire payloads.
- `framerail/tests/form-fields.test.mjs`: bounded Svelte server-rendered radio/dropdown controls, typed selection, unknown/unselected values and static text; not browser interaction proof.
- `framerail/tests/wikitext-wizards.test.ts`: pure insertion/validation coverage for all six wizard forms, including the code-versus-other-wizard selection distinction and equation output modes.
- `framerail/tests/wikitext-wizard-component.test.mjs`: server-rendered wizard defaults/controls and escaped equation source. `aa39c6a` initializes the first equation consistently for SSR and browser. Final helper/component/action tests passed 19/19 (`/tmp/claude/cobalt-wizard-final-targeted-tests-aa39c6a.log`).
- `framerail/tests/editor-lookup.test.ts`: five action contracts at `0a78014`: trusted request context, short-query rejection, mapped results, and safe failures.
- `deepwell/tests/editor_lookup.rs`: native authorized current-page image lookup, image/latest-revision filtering, and anonymous/missing/forged-target rejection; 4/4 at `50c4fd7` (`/tmp/claude/cargo-editor-lookup-50c4fd7.out`).
- `framerail/tests/local/editor-wizards.mjs`: authenticated local browser acceptance 2/2 at `aa39c6a` (`/tmp/claude/cobalt-wizards-browser-aa39c6a.log`): all six insert and preview, real page suggestions, empty/nonempty attachment lookup, loaded selected attachment and rendered image, Flickr invalid/numeric input, missing-target attachment exclusion, cancel focus/selection restoration, unchanged source/revisions, and missing target still absent.
- `deepwell/tests/page_draft.rs`: native draft authorization, save/restore/delete and typed-value coverage; 13 passed at `835eaf7`. `9d20256` adds the confirmed delete response assertion.
- `/tmp/claude/cobalt-page-draft-browser-fifth.log`: authenticated local browser lifecycle 1/1, including unpublished-target save/reopen/restore/publish/delete, structured typed-value preservation, and anonymous denial.
- `/tmp/claude/cobalt-preview-911748e-native.log`: native 6/6 proof of authorized missing/existing raw previews, structured preview merge and stale/invalid rejection, trusted request identity/access before submitted validation, and no revisions from invalid/conflicting preview input.
- `/tmp/claude/cobalt-page-preview-toolbar-browser.log`: browser 1/1 proof that raw create and existing previews use a real bold-toolbar caret/focus interaction and render raw heading/bold HTML; structured Name preview renders; no publication/revision/source/form changes occur; anonymous preview is denied.
- Run with Node's built-in test runner: `node --test tests/form-editor.test.ts tests/form-fields.test.mjs` from Framerail. Uses the existing `jiti` dependency for extensionless TypeScript imports and existing Svelte dependencies; no full application build/server.

## Known gaps (current cycle)

- [ ] Hosted-server ownership, visibility, and lifecycle parity remain unproven. The shared target/ownership model is authorized source-based inference; the captured source check has no Save/Cancel/Delete action.
- [ ] Establish source visual/editor parity: watchers, quick-reference/snippets, source helper behavior, and ACL/history/import integration remain open.
- [ ] Establish source autocomplete parity. The current lookup is permission-filtered current-content search, not a verified title-autocomplete replica.
- [x] Nonempty attachment selection and Preview: `/tmp/claude/cobalt-wizard-attachment-browser-second.log`, 1/1 at `1d06348`; original six-wizard proof remains valid and was not rerun.
- [ ] Flickr photo-info checking remains unimplemented. Size options were not captured and are not implemented.
- [ ] Toolbar transformation behavior is source-backed and the bold path has browser proof, but full visual/editor parity is not established. Independent final verification (386) passed scoped ESLint, Stylelint, Prettier, 19/19 targeted tests and Rust formatting; prior unchanged-backend `cargo check` proof is retained. Global Svelte checking still reports two unrelated errors in `tests/local/form-create.mjs:15,23`; no wizard diagnostics remain. Logs: `/tmp/claude/cobalt-wizard-final-*-aa39c6a.log`. This is not a full-readiness claim.
- The retained `/tmp/claude/cobalt-forms-browser-fourth.log` covers text/wiki/radio/select edits and typed/unknown-value preservation. Restored-draft merge tests and the draft browser scenario add complete-value preservation coverage; they do not establish every field type's full source parity.

## Out of scope

Source-site writes, production deployment without approval, source ACL parity, and unrelated visual redesign are outside this local editor implementation.
