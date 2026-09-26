# Cobalt page files

The Wikidot-layout Files pane presents attached-file information using the existing page-file list. The retained reference is `templates/modules/files/{PageFilesModule,FileInformationWinModule}.tpl` in `/home/osso/Repos/wikidot`.

## What it must do

- [x] Show total bytes for listed nondeleted files, including a zero-byte total for a nonempty active list; omit the total for an empty list.
- [x] Offer each active file a closable, read-only information view with its name, existing file URL, byte size, MIME type, local `file_created_at` value, and nonempty revision comment.
- [x] Label `file_created_at` “Local file created”; it is not the original upload timestamp.
- [x] Use absolute same-origin file URLs through the existing `/-/file` permission boundary; encode filename spaces and fragment markers.
- [x] Keep deleted-file restore actions and non-Wikidot presentation unchanged.

## How it works

- The existing Files pane fetches the page-file list; no additional request is needed for information or totals.
- [Page actions](cobalt-page-actions.md) defines bottom-action registration and its evidence boundary.

## Implementation inventory

- `framerail/src/routes/[slug]/[...extra]/FilePane.svelte` — list, aggregate and information presentation.

## Tests asserting this spec

- `framerail/tests/page-files.test.ts` — four SSR cases for totals, information, same-origin encoded URLs, and alternate layout.
- `framerail/tests/local/remaining-actions.mjs` — local browser information, total, and close coverage.

## Known gaps (current cycle)

- [ ] Upload, restore, and file-mutation authorization remain unproven.
- [ ] Final independent checks remain ongoing; the bounded preservation record excludes only the sacrificial Tags/Parent fixture and native cache fields.

## Out of scope

- Uploader names, original upload timestamps, original upload comments, and legacy content-storage metadata are unavailable in the existing `PageFile` response. `file_created_at` is local creation time; it must not be presented as original upload time. Revision user ID and revision comments must not be presented as uploader name or original comment.
- Upload limits, new backend requests, mutation flows and row-action redesign are not part of this presentation slice.
