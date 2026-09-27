# Cobalt page files

The Wikidot-layout Files pane presents attached-file information using the existing page-file list and supports new uploads on the current page. The retained reference is `templates/modules/files/{PageFilesModule,FileInformationWinModule}.tpl` in `/home/osso/Repos/wikidot`; its uploader is also single-file, so this contract makes no claim about historical batch support.

## What it must do

- [x] Show total bytes for listed nondeleted files, including a zero-byte total for a nonempty active list; omit the total for an empty list.
- [x] Offer each active file a closable, read-only information view with its name, existing file URL, byte size, MIME type, local `file_created_at` value, and nonempty revision comment.
- [x] Label `file_created_at` “Local file created”; it is not the original upload timestamp.
- [x] Use absolute same-origin file URLs through the existing `/-/file` permission boundary; encode filename spaces and fragment markers.
- [x] Keep deleted-file restore actions and non-Wikidot presentation unchanged.
- [x] Dispatch file revision rollback from the page route to the existing RPC, preserving the request IP and accepting its revised file response.
- [x] Serialize an omitted rollback comment as an empty string for the RPC wire contract.
- [x] Complete the disposable Files lifecycle: upload, rename, byte replacement, revision-history read, rollback, move, delete, restore, and final deletion. The run preserves original fixture files and pages.
- [x] Let the current-page uploader select one or more files. A single file retains optional rename and comment fields and closes on success; a multi-file selection uploads each original filename with one shared comment.
- [x] Submit a selected batch sequentially through the existing same-page `?/fileUpload` action, not a backend batch API. Show each file's status or error and refresh the listing after completed uploads.
- [x] Preserve existing Page/Edit/Block mutation permissions, limits, and collision behavior. Do not implicitly overwrite or retry. A failed file leaves earlier successes intact and later files continue; successful entries are not sent again until the user selects a new batch.
- [x] Govern upload, rename/edit, byte replacement, revision-history read, rollback, delete, and restore with the page's `Page/Edit` permission. `fileMove` requires `Page/Edit` on both its source and destination pages. No distinct File grant exists or is required. `deepwell/tests/file_permission.rs` passes 3/3 after `e974e6f84` initializes rollback creation revision at `0` (`/tmp/claude/cobalt-move-file-targeted.log`).
- [x] Apply the additional active Wikidot Block policy to file mutations implemented in `endpoints/file.rs`, including rollback. `endpoints/file_revision.rs` deliberately authorizes revision-history read, range, and count with owning-page `Page/Edit` only; history read is not a mutation and does not apply Block.
- [x] Omit revision comments marked hidden from the information view; this does not change the existing backend file-list payload.

## How it works

- The existing Files pane fetches the page-file list; no additional request is needed for information or totals.
- A batch remains client-side sequential requests to the existing upload route; each request keeps the existing per-file contract.
- [Page actions](cobalt-page-actions.md) defines the shared Page/Edit and Block mutation-policy baseline and its evidence boundary.

## Implementation inventory

- `framerail/src/routes/[slug]/[...extra]/FilePane.svelte` — list, aggregate, information, and uploader placement.
- `framerail/src/routes/[slug]/[...extra]/FileUploadForm.svelte` — single- and multi-file upload form.
- `framerail/src/lib/fileUploadBatch.ts` — sequential per-file upload state and receipt handling.

## Tests asserting this spec

- `framerail/tests/page-files.test.ts` — five SSR cases for totals, information, hidden comments, same-origin encoded URLs, and alternate layout.
- `framerail/tests/page-file-rpc.test.ts` — rollback route dispatch, request-IP serialization, revised-response acceptance, and empty omitted-comment wire contract.
- `framerail/tests/local/remaining-actions.mjs` — local browser information, total, and close coverage.
- `framerail/tests/local/file-action-mutations.mjs` — disposable browser file-mutation lifecycle; main GREEN 1/1 covers upload, rename, byte replacement, history read, rollback, move, delete, restore, final deletion, and original-fixture preservation.
- `deepwell/tests/file_permission.rs` — endpoint-authorization coverage; targeted GREEN 3/3 after `e974e6f84` initializes rollback creation revision `0`.
- `framerail/tests/file-upload-form.test.ts` — seven unit cases for single and batch selection, request sequencing, shared comments, collision retention, receipt handling, and completion state.
- `framerail/tests/local/batch-upload.mjs` — local browser batch lifecycle.
- `framerail/tests/local/file-action-transport.test.mjs` — native multipart request-guard coverage.

## Evidence

- `/tmp/claude/cobalt-files-hidden-comment-green.log` — five SSR cases pass after a real RED case: totals, information, hidden-comment omission, encoded URLs, and alternate layout. The existing backend file-list payload is unchanged; omission is presentation-only, not an API-redaction claim.
- `/tmp/claude/cobalt-file-info-final.png` — local captured file-information state; presentation evidence only.
- `cb61f5543` — file edit and rollback now send `getClientAddress()` as RPC `ip_address`. `framerail/tests/page-file-rpc.test.ts` recorded targeted RED then GREEN 2/2 for serialized edit/rollback request IP and accepted revisions; this is server/RPC proof, not a browser lifecycle pass.
- `ca141f0b3` — separates file-action transport assertions from the browser lifecycle test. This readability split supplies no browser proof.
- `90df39567` — permits byte replacement when the supplied name equals that file's current revision name; a different file's same-page name still conflicts.
- `/tmp/claude/cobalt-file-self-conflict-{red,green}.log` and `/tmp/claude/cobalt-file-self-conflict-gate.json` — targeted regression RED then GREEN, with independent PASS: unchanged-name replacement stores replacement bytes, while another file's same-page name still conflicts. The gate also records Rust format/check and changed-code readability scope.
- `/tmp/claude/cobalt-file-self-conflict-runtime.json` — runtime SHA-256 matches the build for `90df3956790558dab6871d4980f56a42c77ca457`; this is runtime identity evidence, not browser proof.
- `/tmp/claude/cobalt-file-self-conflict-deploy.log` — reported local deploy exit `0`; retained file is empty, so it independently proves no deployment detail and is not browser lifecycle proof.
- `2d9d9989a` — registers the existing `fileRollback` page action; `framerail/tests/page-file-rpc.test.ts` asserts route dispatch to `file_rollback` and its revised response.
- `fc90bbe98` — serializes omitted rollback comments as `""`; the same RPC test asserts that wire value.
- `/tmp/claude/cobalt-file-action-rollback-comments.log` — earlier browser prefix reaches upload, rename, byte replacement, history read, and rollback; its Move guard decoder blocked dispatch. Superseded as lifecycle evidence by the main GREEN below; the decoder failure was not application-failure evidence.
- `2ee59f4de` — makes the test transport guard decode URL-encoded Superforms mutations, allowing the guarded lifecycle to reach its actual application requests.
- `/tmp/claude/cobalt-file-action-urlencoded.log` — actual main Files lifecycle GREEN 1/1 at `2ee59f4de`: upload, rename, byte replacement, history read, rollback, move, delete, restore, final deletion, and original fixture file/page invariants; no blocked or failed requests, page errors, or external reads.
- `/tmp/claude/cobalt-file-decode-final-gate.json` — independent PASS at `e09e5ae7f`: whole-project `svelte-check` has 0 errors/0 warnings; scoped ESLint and Prettier pass for the changed transport test; readability passes after extracting the decode-error classifier (`decodeMutation` cognitive complexity 11, below 15). It reuses the unchanged actual Files lifecycle browser GREEN from `/tmp/claude/cobalt-files-frontend-followup.json`; no browser rerun occurred.
- `/tmp/claude/cobalt-action-preservation-gate.json` — read-only preservation PASS for pages, history, drafts, grants, and native content; Files are explicitly not covered.
- `/tmp/claude/cobalt-move-file-targeted.log` — file authorization GREEN 3/3, alongside Move dependency coverage 4/4, after `e974e6f84` fixes rollback creation revision initialization.
- `/tmp/claude/cobalt-page-file-auth-runtime.json` — local auth deployment receipt predating Move commits. It is runtime identity evidence, not a post-change browser authorization proof.
- `/tmp/claude/cobalt-move-rpc-final-green.log` — historical Move RPC/UI serialization GREEN 18/18 after `facf4cf28` and `4367827bc`.
- `/tmp/claude/cobalt-current-file-action-mutations.log` — current browser lifecycle GREEN 1/1: upload, rename, byte replacement, history read, rollback, move, delete, restore, final deletion, and original fixture file/page preservation.
- `/tmp/claude/cobalt-page-action-denials-textplain.log` — observer browser denial covers file Delete/Edit/Move only; the remaining file-denial actions are not browser claims.
- `/tmp/claude/cobalt-final-backend-followup.json` — current independent backend follow-up PASS reuses file-history authorization 3/3 and combined mutation/Block coverage 27/27; local runtime/build identity is recorded in `/tmp/claude/cobalt-final-actions-runtime.json` and is not production.
- `/tmp/claude/cobalt-final-action-preservation.json` — refreshed SQL preservation preserves pages, imported history, native content, grants, and drafts, but excludes Files; this lifecycle separately asserts original fixture file/page preservation.
- `/tmp/claude/cobalt-batch-receipt-green.log` — unit GREEN 6/6 after receipt-missing and false-success RED cases; an empty upload error is not success.
- `/tmp/claude/cobalt-batch-upload-browser-row-fixed.log` — local batch GREEN 1/1: first and third files retain bytes and shared comment; the duplicate middle filename remains a collision; originals are preserved and cleanup completes.
- `/tmp/claude/cobalt-batch-upload-guard.log` — native multipart action guard GREEN 12/12.
- **Production application rollout, 2026-09-26:** Files-related application code is included in released revision `16ffc05e2b8049b90e0de9324741522d518857e7`, deployed by `install/dev-deploy.sh all` to Deepwell and Framerail only (`/tmp/claude/cobalt-actions-production-deploy.log`). Independent final gate `/tmp/claude/cobalt-actions-production-final-gate.json` is `PASS`, superseding the original keyword-only journal `FAIL` gate; it confirms installed hashes, active services, public routes, anonymous `can_edit=false`, watching-read denial, and co-tenant readiness. The public proof remains read-only, with no production test-data mutations; it does not prove the Files lifecycle or authenticated file authorization in production.

## Known gaps (current cycle)

- [ ] Browser observer denial proves only file Delete/Edit/Move. Upload, revision-history read, rollback, restore, and other file-denial actions retain isolated authorization proof; do not represent them as browser proof.
- [ ] The lifecycle preserves only disposable/original fixture files and pages. The refreshed SQL comparison excludes Files, three page identities, and renderer fields. The 43 unreconstructable original full-row fingerprints remain outside full-row preservation claims; they are not SQL row exclusions.
- [x] Automated local browser proof passes: batch 1/1 (`/tmp/claude/cobalt-batch-final-batch-upload.log`) and full single-file lifecycle 1/1 (`/tmp/claude/cobalt-batch-final-file-action-mutations.log`), including persisted bytes and fixture cleanup.
- [x] Independent frontend follow-up and final style receipt pass: `/tmp/claude/cobalt-batch-upload-final-followup.json` and `/tmp/claude/cobalt-batch-final-style-gate.json`. Svelte, scoped lint, formatting, styles, and readability checks pass; helper 7/7 and transport 12/12 proof retained.
- Batch upload is not production: release `16ffc05e2b8049b90e0de9324741522d518857e7` predates it. The user's visible browser-cli demonstration remains separate from the automated browser proof.
- [x] Current frontend checks and formatting follow-up pass; receipts and inherited-warning scope are recorded in [page actions](cobalt-page-actions.md). They do not expand this action-specific authorization boundary.

## Out of scope

- Uploader names, original upload timestamps, original upload comments, and legacy content-storage metadata are unavailable in the existing `PageFile` response. `file_created_at` is local creation time; it must not be presented as original upload time. Revision user ID and visible revision comments must not be presented as uploader name or original comment.
- New backend batch requests and row-action redesign are out of scope. Existing upload limits, permissions, collision behavior, and per-file request handling remain unchanged.
