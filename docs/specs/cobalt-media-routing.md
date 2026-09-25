# Same-site media routing

FTML must render same-site attachment references through the deployed site's protected file route, not an unconfigured external hostname. See [replica status](../wiki/systems/cobalt-replica-status.md).

## What it must do

- [x] Emit same-origin file URLs for current-page, same-site other-page, and explicitly same-site references. A bare current-page reference retains its complete canonical page name: on `character:melancholy`, `[[image Melancholy_Outfits]]` must resolve to `/-/file/character:melancholy/Melancholy_Outfits`, never the root `melancholy` owner; later colons remain intact.
- [x] Preserve explicit external URLs and unrelated cross-site references.
- [x] Preserve source bytes and revision identity while refreshing derived HTML.
- [x] Serve actual imported image bytes behind authentication and no-index controls.
- [x] Render Wikidot `[[gallery]]` markup: bare galleries list the page's image attachments in Wikidot's en_US name order, `: file` lists keep source order; originals are shown at Wikidot's thumbnail sizes because there is no resizer.
- [x] An ordinary thumbnail click opens that gallery's image viewer without navigating away. Show the original image at natural size within the viewport, its position/count, bounded Previous/Next controls, and Close/Escape dismissal with focus returned to the thumbnail. Opening/browsing/closing must not save page content; modified clicks retain normal link behavior.
- [x] Resolve files on pages whose names have more than one colon (`/-/file/writing:2021-10-21-to-paint-a-picture:the-game/chessset.jpg`): wws normalizes page slugs with the same patched `wikidot-normalize` as Deepwell.
- [x] Serve a page's files (`/-/file`, `/-/download`) and text blocks (`/-/code`, `/-/html`) only to viewers the page view would show the page to; others get `403` (the page view's status), `Cache-Control: private, no-store`. The viewer is the `wikijump_token` session cookie; an unknown or expired token counts as anonymous.
- [x] Files of pages anonymous visitors may view stream from wws with `Cache-Control: public, max-age=2592000` (30 days). A file replaced under the same name can be served stale from browser/edge caches for up to 30 days.
- [x] Files of restricted pages: a permitted viewer gets `302` to a presigned S3 GET URL (7-day SigV4 expiry, signed at the start of the UTC day so the URL is the same all day and at least 6 days remain; `response-content-type` = the file's MIME type, `response-content-disposition` inline with the original filename, attachment on `/-/download`); the redirect is `private, no-store`. The URL addresses the content hash, so a replaced file never serves stale bytes. The object store handles range requests.
- [x] Text blocks of restricted pages are `private, no-store`; public text blocks keep no cache lifetime because their URL survives page edits.

## How it works

- [Protected file gateway](cobalt-poc-gateway.md)
- Request order in wws: page lookup (cached) → Deepwell `page_view_permission` (not cached, per viewer) → file lookup (cached) → stream or redirect. Denial happens before the file lookup, so a private page's filenames are not probed.

## Implementation inventory

- `framerail/src/lib/gallery.ts`, `framerail/src/lib/component/GalleryViewer.svelte`, `framerail/src/routes/+layout.svelte`: gallery-scoped click selection, modal viewer and capture-phase integration before client routing.
- `deepwell/vendor/ftml/src/render/handle.rs`: selects same-origin routes for same-site media.
- `wws/Cargo.toml` `[patch.crates-io]`: `deepwell/vendor/wikidot-normalize`; the Nix `wws` source (`install/nixos/packages.nix`) and the wws Dockerfiles include that directory.
- `deepwell/src/services/view/service.rs` `ViewService::page_view_permission` (RPC `page_view_permission`, params `site_id`, `page_id`, `session_token`) → `{can_view, public}`: the page view's Page/View check (page's category, page context) for the session's user and for anonymous.
- `wws/src/visibility.rs`: session cookie, `PageVisibility`, cache headers, `403`. `wws/src/presign.rs`: day-rounded presigned GET (rust-s3 `ReqwestRequest` with its `datetime` set; query parameters sorted because rust-s3 emits custom queries in `HashMap` order). `wws/src/handler/file.rs`: stream vs redirect.

## Tests asserting this spec

- `deepwell/tests/page_media_urls.rs`: native rendering of local, explicit same-site, external and cross-site image references, including bare category-page media and the live `character:melancholy` ImageBox path.
- `deepwell/tests/page_gallery.rs` and FTML `render::html::element::gallery` tests: gallery markup, sizes, order and image filtering.
- `framerail/tests/local/gallery.mjs`: actual Badges thumbnail click, original image, Next/Previous, Close/Escape, keyboard reopening, last-image boundary, unchanged URL/source/revision and zero write requests.
- FTML AST fixtures: image/audio/video output contracts.
- `deepwell/tests/page_view_permission.rs` (DB): public vs private category, member vs anonymous vs banned member vs unknown token, page from another site rejected.
- `wws` unit tests: `fetch` (multi-colon slug kept), `visibility` (cookie, decision, `403` headers), `presign` (same URL within a UTC day, new URL next day, expiry, overrides, reserved characters stay encoded), `attachment` (inline disposition).
- `wws/src/handler/file_access_tests.rs` (`--ignored`, needs `WWS_TEST_REDIS_URL`): real router and Redis against a fake Deepwell; private-page file `403` anonymous/other session, `302` presigned for the member (stable URL, download disposition); public multi-colon page file `200` with the 30-day public cache.
- `wws/src/presign.rs` `store_serves_presigned_url_with_overrides` (`--ignored`, needs an S3 store): the store accepts the day-rounded signature and returns the overridden type and disposition.

## Known gaps (current cycle)

- [x] Native routing test and all 121 FTML AST fixtures pass; fmt/check pass. Deployed and refreshed 28 affected pages, preserving source hashes and revision identity. Browser loads both homepage images backed by imported attachments, with matching hashes through public authenticated HTTPS.
- [ ] Native gallery viewing (`cef28e93b`, `01e1bfaa4`, formatting/types `28b30c9cc`) is artifact-deployed through shared Framerail HEAD `3f9c20249b082416a8c661bab34685d4fcf67c31`. Local and production build trees match (500 files; SHA-256 `bc8daab87d1aa10bf781395e111ba8380f330de3ba8daaea34587a810c315359`; `/tmp/claude/cobalt-badges-production-artifact.json`). Component/browser tests passed 3/3 (`/tmp/claude/cobalt-badges-node-tests.log`); main inspected `/tmp/claude/cobalt-badges-viewer-fixed.png`. Independent public browser verifier 32 remains active: browser proof is pending, so production click behavior is not yet claimed. Five existing imported-history type errors remain outside this scope.
- [ ] Two homepage images still depend on unresolved template variables; routing must not pretend their requested assets exist.
- [x] Local full-preview image correction browser proof passed: authenticated `home:_public` (`2/2` visible images, `2/2` CSS backgrounds, `4/4` verified assets) and `home:start` (`19/19`, `3/3`, `22/22`) have no failures; unauthenticated access remains `401` and no-index remains present. The [replica-status SSOT](../wiki/systems/cobalt-replica-status.md#local-full-preview-media-routing) records causes and evidence scope.

- [x] `62af8b88a` preserves category ownership for bare current-page media and is deployed in production build `74f1e36abd76`. Public browser proof covers Melancholy's portrait and six gallery assets, roster thumbnail/navigation, and category-qualified images on Isla, Avrenne, and Baird; the [replica-status SSOT](../wiki/systems/cobalt-replica-status.md#current-page-media-owner-fix-2026-09-25) retains scope and the unresolved `icon_tanith.jpg` source absence.
- [ ] File access and caching are committed, not deployed; see [replica status](../wiki/systems/cobalt-replica-status.md#wws-file-access-2026-09-24) for proofs and the post-deploy checks.
- [ ] Wikidot itself serves files and text blocks of private pages to anonymous visitors (checked 2026-09-24: `admin:css` page shows "Private content", while `/local--files/admin:css/liberty-webfont.woff2` returns `200` and `/admin:css/code/1`, the site theme CSS, returns `200`). The replica deliberately refuses them; the replica's theme comes from `/-/cobalt-theme.css`, so it does not depend on `admin:css`.
- [ ] Restricted files loaded cross-origin by CSS or scripts (fonts, `fetch`) after the redirect need a CORS policy on the R2 bucket; `<img>`, links and downloads do not.
- [ ] Gallery pages rendered before gallery support (`icons`, `badges`, `images`, `pet-icons`, `membersonly:player-icons`) need a refresh after deployment.

## Out of scope

External asset acquisition, cross-site routing redesign, template expansion, full visual parity (including top navigation tabs and sidebar matching the original), and source ACL parity (which roles may view which categories) remain separate work; wws enforces whatever Deepwell's Page/View permissions say. Shared POC authentication stays mandatory.
