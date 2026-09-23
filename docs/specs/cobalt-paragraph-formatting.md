# Cobalt paragraph formatting

Cobalt's Wikidot-layout renderer must preserve paragraph boundaries needed by imported Wikidot sources. Current local comparison target: `who-we-are`, page `3000003984`, site `6000000`.

## What it must do

- [x] Render a Wikidot-layout paragraph beginning with an image without a `p` wrapper.
- [x] Retain the `p` wrapper when text precedes an inline image.
- [x] Strip a completed Wikidot comment after include expansion, before FTML preprocessing and rendering.
- [x] When stripping a completed comment, consume its immediately preceding LF without changing raw-protected markers or non-ASCII text.
- [x] Preserve ImageBox's image, one break, and caption without a paragraph wrapper through include → comment stripping → preprocessing → rendering.
- [x] Serve successful Wikidot content-root and slug documents with the source-compatible XHTML 1.0 Transitional doctype, while native, error, and special responses remain HTML5.

## How it works

- [Native runtime](cobalt-native-runtime.md)
- [POC import](cobalt-poc-import.md)
- [Replica status](../wiki/systems/cobalt-replica-status.md#local-who-we-are-paragraph-proof)

## Implementation inventory

- `deepwell/vendor/ftml/src/render/html/element/container.rs`: Wikidot-layout paragraph HTML rendering.
- `deepwell/src/services/render/wikidot_comments.rs`: Wikidot comment stripping before FTML rendering.
- `framerail/src/hooks.server.ts`: response doctype selection.
- `framerail/src/routes/+page.server.ts`: Wikidot content-root document classification.
- `framerail/src/routes/[slug]/[...extra]/+page.server.ts`: Wikidot slug-document classification.

## Tests asserting this spec

- `deepwell/vendor/ftml/src/render/html/element/container.rs`: leading-image, inline-image, and prose paragraph regression.
- `deepwell/src/services/render/wikidot_comments.rs`: four comment/raw/UTF-8 regressions and one faithful ImageBox include/render regression.
- Existing FTML regression: one test; existing list regressions: two tests.
- `framerail/tests/page-doctype.test.ts`: four focused response-classification/doctype regressions.

## Proof ledger

- `2ffcebe` implements the checked rendering/comment requirements. The comment behavior follows `gabrys/wikidot` `Parse/Default/Comment.php` for the preceding LF.
- `c0d79e1` selects XHTML 1.0 Transitional only for successful Wikidot content-root and slug documents. Its four focused response tests passed; native, error, and special responses remain HTML5.
- Local import target source is unchanged: 4,758 bytes, SHA-256 `81faa6b5a13d43bd59177409909c4c650df86550cd60601f7db36d54da260bbe`. Three source images are loaded with matching MIME type and SHA-256; visible text matches.
- At current code, `/tmp/claude/cobalt-who-we-are-browser-green.log` passes: `who-we-are` returns HTTP 200; visible-text SHA matches source; all three images match source MIME/SHA and exact positions; at 1440×1000, first caption y=670, ImageBox height=289, and article height=1605; sidebar is absent; browser reports no JavaScript errors. The standalone identical-HTML probe measured the source XHTML caption gap as 5 px and HTML5 as 13 px.
- Main rebuilt Deepwell `2749` and rerendered only `who-we-are`; source and revision remain unchanged.

## Known gaps (current cycle)

- [ ] Independent final verification of the current browser proof remains pending.
- [ ] Broader archived-page/browser comparison remains unproven; this `who-we-are` result does not establish full-site parity or readiness.

## Out of scope

- Full-site parity/readiness, deployment, and source ACL parity.
- Search service choice, including proposed Meilisearch, is a separate decision; no search implementation is included.
