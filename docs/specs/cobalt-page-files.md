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
- [ ] Upload, rename, byte replacement, revision-history read, and rollback have observed browser progress, but the encompassing lifecycle test is not green because its Move test guard decoder blocks dispatch; this is not app-failure evidence.
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
- `framerail/tests/local/file-action-mutations.mjs` — disposable browser file-mutation lifecycle; latest run reaches upload, rename, byte replacement, history read, and rollback before its Move guard decoder blocks dispatch.

## Evidence

- `/tmp/claude/cobalt-files-hidden-comment-green.log` — five SSR cases pass after a real RED case: totals, information, hidden-comment omission, encoded URLs, and alternate layout. The existing backend file-list payload is unchanged; omission is presentation-only, not an API-redaction claim.
- `/tmp/claude/cobalt-file-info-final.png` — local captured file-information state; presentation evidence only.
- `cb61f5543` — file edit and rollback now send `getClientAddress()` as RPC `ip_address`. `framerail/tests/page-file-rpc.test.ts` recorded targeted RED then GREEN 2/2 for serialized edit/rollback request IP and accepted revisions; this is server/RPC proof, not a browser lifecycle pass.
- `ca141f0b3` — separates file-action transport assertions from the browser lifecycle test. This readability split supplies no browser proof.
- `90df39567` — permits byte replacement when the supplied name equals that file's current revision name; a different file's same-page name still conflicts.
- `/tmp/claude/cobalt-file-self-conflict-{red,green}.log` — targeted regression RED then GREEN: unchanged-name replacement previously failed with the file's own conflict; it now stores replacement bytes and preserves another file-name conflict.
- `/tmp/claude/cobalt-file-self-conflict-deploy.log` — local deploy exited `0`; this is not browser lifecycle proof.
- `2d9d9989a` — registers the existing `fileRollback` page action; `framerail/tests/page-file-rpc.test.ts` asserts route dispatch to `file_rollback` and its revised response.
- `fc90bbe98` — serializes omitted rollback comments as `""`; the same RPC test asserts that wire value.
- `/tmp/claude/cobalt-file-action-rollback-comments.log` — browser run reaches upload, rename, byte replacement, history read, and rollback. Its Move request is blocked by the test guard decoder before dispatch (`mutation decode failed`), so the run is not a green full-lifecycle result and does not establish an application failure.

## Known gaps (current cycle)

- [ ] The `cb61f5543` edit/rollback request-IP fix has targeted RED/GREEN 2/2 RPC proof, `90df39567` has targeted RED/GREEN proof for unchanged-name byte replacement while preserving a different-file conflict, and `2d9d9989a`/`fc90bbe98` have targeted rollback route/wire proof. The latest browser run observes upload, rename, byte replacement, history read, and rollback, but its Move guard decoder blocks dispatch. Full Files remains open: Move, delete, restore, file-mutation authorization, and a green end-to-end lifecycle are unproven; local deployment is not lifecycle proof.
- [x] The independent frontend gate passes, including the hidden-comment fixture; see [page actions](cobalt-page-actions.md). The bounded preservation record excludes only the sacrificial Tags/Parent fixture and native cache fields.

## Out of scope

- Uploader names, original upload timestamps, original upload comments, and legacy content-storage metadata are unavailable in the existing `PageFile` response. `file_created_at` is local creation time; it must not be presented as original upload time. Revision user ID and visible revision comments must not be presented as uploader name or original comment.
- Upload limits, new backend requests, and row-action redesign are not part of this presentation slice. Existing mutation flows are tracked only to the bounded proof stated above; this contract does not expand authorization or full Files lifecycle scope.
