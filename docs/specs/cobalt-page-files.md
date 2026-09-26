# Cobalt page files

The Wikidot-layout Files pane presents attached-file information using the existing page-file list. The retained reference is `templates/modules/files/{PageFilesModule,FileInformationWinModule}.tpl` in `/home/osso/Repos/wikidot`.

## What it must do

- [x] Show total bytes for listed nondeleted files, including a zero-byte total for a nonempty active list; omit the total for an empty list.
- [x] Offer each active file a closable, read-only information view with its name, existing file URL, byte size, MIME type, local `file_created_at` value, and nonempty revision comment.
- [x] Label `file_created_at` “Local file created”; it is not the original upload timestamp.
- [x] Use absolute same-origin file URLs through the existing `/-/file` permission boundary; encode filename spaces and fragment markers.
- [x] Keep deleted-file restore actions and non-Wikidot presentation unchanged.
- [x] Dispatch file revision rollback from the page route to the existing RPC, preserving the request IP and accepting its revised file response.
- [x] Serialize an omitted rollback comment as an empty string for the RPC wire contract.
- [x] Complete the disposable Files lifecycle: upload, rename, byte replacement, revision-history read, rollback, move, delete, restore, and final deletion. The run preserves original fixture files and pages.
- [x] Omit revision comments marked hidden from the information view; this does not change the existing backend file-list payload.

## How it works

- The existing Files pane fetches the page-file list; no additional request is needed for information or totals.
- [Page actions](cobalt-page-actions.md) defines bottom-action registration and its evidence boundary.

## Implementation inventory

- `framerail/src/routes/[slug]/[...extra]/FilePane.svelte` — list, aggregate and information presentation.

## Tests asserting this spec

- `framerail/tests/page-files.test.ts` — five SSR cases for totals, information, hidden comments, same-origin encoded URLs, and alternate layout.
- `framerail/tests/page-file-rpc.test.ts` — rollback route dispatch, request-IP serialization, revised-response acceptance, and empty omitted-comment wire contract.
- `framerail/tests/local/remaining-actions.mjs` — local browser information, total, and close coverage.
- `framerail/tests/local/file-action-mutations.mjs` — disposable browser file-mutation lifecycle; main GREEN 1/1 covers upload, rename, byte replacement, history read, rollback, move, delete, restore, final deletion, and original-fixture preservation.

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

## Known gaps (current cycle)

- [ ] The `cb61f5543` edit/rollback request-IP fix has targeted RED/GREEN 2/2 RPC proof, `90df39567` has targeted RED/GREEN self-conflict proof, and `2d9d9989a`/`fc90bbe98` have targeted rollback route/wire proof. Main browser proof now covers the complete disposable Files lifecycle, but file-mutation authorization and action-specific denials remain unproved; lifecycle proof does not establish them.
- [ ] The current frontend gate remains pending. Preservation refresh is also pending: the prior bounded record predates the disposable Files fixtures and excludes sacrificial Tags/Parent fixtures and native cache fields; this lifecycle's original-fixture invariants do not refresh that broader record.

## Out of scope

- Uploader names, original upload timestamps, original upload comments, and legacy content-storage metadata are unavailable in the existing `PageFile` response. `file_created_at` is local creation time; it must not be presented as original upload time. Revision user ID and visible revision comments must not be presented as uploader name or original comment.
- Upload limits, new backend requests, and row-action redesign are not part of this presentation slice. Existing mutation flows are tracked only to the bounded proof stated above; this contract does not expand authorization or full Files lifecycle scope.
