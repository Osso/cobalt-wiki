# Cobalt page files

The Wikidot-layout Files pane presents attached-file information using the existing page-file list. The retained reference is `templates/modules/files/{PageFilesModule,FileInformationWinModule}.tpl` in `/home/osso/Repos/wikidot`.

## What it must do

- [x] Show total bytes for listed nondeleted files, including a zero-byte total for a nonempty active list; omit the total for an empty list.
- [x] Offer each active file a closable, read-only information view with its name, existing file URL, byte size, MIME type, creation date and nonempty revision comment.
- [x] Use absolute same-origin file URLs through the existing `/-/file` permission boundary; encode filename spaces and fragment markers.
- [x] Keep deleted-file restore actions and non-Wikidot presentation unchanged.

## How it works

- The existing Files pane fetches the page-file list; no additional request is needed for information or totals.

## Implementation inventory

- `framerail/src/routes/[slug]/[...extra]/FilePane.svelte`: list, aggregate and information presentation.

## Tests asserting this spec

- `framerail/tests/page-files.test.ts`: nonempty SSR fixtures for totals, file information and alternate layout.

## Known gaps (current cycle)

- [ ] Live nonempty browser rendering and close interaction remain unverified; the audited live fixture had no files.

## Out of scope

- Uploader names, original upload comments and legacy content-type descriptions are unavailable in the existing `PageFile` response; revision user ID and revision comments must not be presented as uploader name or original comment.
- Upload limits, new backend requests, mutation flows and row-action redesign are not part of this presentation slice.
