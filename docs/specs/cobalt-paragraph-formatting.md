# Cobalt paragraph formatting

Cobalt's Wikidot-layout renderer must preserve paragraph boundaries needed by imported Wikidot sources. Current local comparison target: `who-we-are`, page `3000003984`, site `6000000`.

## What it must do

- [x] Render a Wikidot-layout paragraph beginning with an image without a `p` wrapper.
- [x] Retain the `p` wrapper when text precedes an inline image.
- [x] Strip a completed Wikidot comment after include expansion, before FTML preprocessing and rendering.
- [x] When stripping a completed comment, consume its immediately preceding LF without changing raw-protected markers or non-ASCII text.
- [x] Preserve ImageBox's image, one break, and caption without a paragraph wrapper through include → comment stripping → preprocessing → rendering.

## How it works

- [Native runtime](cobalt-native-runtime.md)
- [POC import](cobalt-poc-import.md)

## Implementation inventory

- `deepwell/vendor/ftml/src/render/html/element/container.rs`: Wikidot-layout paragraph HTML rendering.
- `deepwell/src/services/render/wikidot_comments.rs`: Wikidot comment stripping before FTML rendering.

## Tests asserting this spec

- `deepwell/vendor/ftml/src/render/html/element/container.rs`: leading-image, inline-image, and prose paragraph regression.
- `deepwell/src/services/render/wikidot_comments.rs`: four comment/raw/UTF-8 regressions and one faithful ImageBox include/render regression.
- Existing FTML regression: one test; existing list regressions: two tests.

## Proof ledger

- `2ffcebe` implements the checked rendering/comment requirements. The comment behavior follows `gabrys/wikidot` `Parse/Default/Comment.php` for the preceding LF.
- Local import target source is unchanged: 4,758 bytes, SHA-256 `81faa6b5a13d43bd59177409909c4c650df86550cd60601f7db36d54da260bbe`. Three source images are loaded with matching MIME type and SHA-256; visible text matches.
- Main rebuilt Deepwell `2749` and rerendered only `who-we-are`; source and revision remain unchanged.

## Known gaps (current cycle)

- [ ] Browser parity remains red at `d921960`: article height is 1,610 px locally versus 1,605 px in the source. Caption gap is 13 px locally versus 5 px because the documents use HTML5 and XHTML Transitional modes respectively.
- [ ] Per-Wikidot doctype rendering is under implementation by agent 293 and is not committed; do not mark the page comparison complete.

## Out of scope

- Full-page parity, deployment, source ACL parity, and any search service or Meilisearch implementation. Search is proposed design only.
