# Cobalt replica status

Verified: 2026-09-23. This records evidence, not authorization to expose source-private content.

## Coverage

| Capability | Proven | Remaining |
|---|---|---|
| [Full POC import](../../specs/cobalt-poc-import.md) | 6,092 active pages and 1,471 attachments; importer exited successfully after byte/hash readback. Independent SQL comparison found no page name/title/tag/size/SHA-256 mismatches, missing/extra records, or wrong migration attribution. Attachment ownership/name/size inventory also matches. | Source authorship, creation dates, full revision/forum history and account mapping |
| [Source theme](../../specs/cobalt-poc-theme.md) | Archived CSS/font served behind authentication; source-image URLs resolve locally and three theme images match archive hashes over public HTTPS. Browser confirms source body/title fonts, black background and 1,000px container. | Full visual/layout parity and dynamic content rendering |
| [Native runtime](../../specs/cobalt-native-runtime.md) and [gateway](../../specs/cobalt-poc-gateway.md) | Native NixOS deployment, public HTTPS, loopback services, protected runtime secrets, Basic authentication, no-index headers, successful Sakuin readiness checks | Full source page/file ACL parity; forged invalid application-session cookies still cause an error |
| [Metadata acquisition](../../specs/cobalt-page-metadata.md) | Listing completed 277/277 pages; metadata finished with 6,090 accepted records, one denied and one redirect | Complete ACL/history/creator acquisition; neither unresolved record is fabricated |
| [Forms](../../specs/cobalt-data-form-schema.md) and rendering | Whole-record schema/value handling, authorized form edits, native DB regressions; all archived sources parse/render without panic. [Automatic link titles](../../specs/cobalt-link-titles.md) now use actual target revisions; four affected cached pages refreshed without new revisions. [Local same-site include expansion](../../specs/cobalt-includes.md) passed six native cases: nested/empty-value body and nav expansion, unavailable/denied/foreign isolation, foreign dependency exclusion, and cycle/depth preservation. `c7859d0` integrates the user-owned rendering branch locally: ListPages, live templates, show-to handling, and FTML fixes now build locally. | Hydrated form save/reload, local browser validation of homepage/navigation/ListPages, pagination/invalidation, source include grammar/SUO support, included-page/permission invalidation, inline-file domain routing and multi-colon reference normalization |

## Access boundary

`cobalt-company.sakuin.org` is publicly reachable over HTTPS without a client tunnel. The existing Sakuin Cloudflare Tunnel connects to the loopback gateway. Every visitor still needs the shared POC login. Public tests cover page, CSS, file, download, robots and `.well-known` authentication challenges with `X-Robots-Tag: noindex, nofollow, noarchive`.

WWS does not implement complete per-page authorization. The shared gate grants its holders access to the whole POC; it must remain until source permission parity is proven and removal is authorized. Source creator identities remain unacquired, so target author/time records explicitly describe the technical migration, not original authorship.

## Local-first development workflow

Use the local preview for rendering, form, and queue work. It avoids production deployment and Nix rebuilds while code is changing.

| Component | Local endpoint/state | Management |
|---|---|---|
| Deepwell | `127.0.0.1:2748`; `cobalt_test` on PostgreSQL `25432` | `systemctl --user status|restart cobalt-local-deepwell` |
| Framerail | `127.0.0.1:5173`; hot reload | `systemctl --user status|restart cobalt-local-framerail` |
| Authenticated gateway | `127.0.0.1:3089` | `systemctl --user status|restart cobalt-local-gateway` |
| Storage | Silo `127.0.0.1:29000` | RAM-backed `cobalt-test-silo-ram`; do not restart during current file proof |
| Queue/cache | Valkey `127.0.0.1:26381`, database `1` | preview-only database; test harness uses database `0` |

Reuse `cobalt-test-postgres` and `cobalt-test-valkey`; do not start parallel copies. The disk-backed `cobalt-test-silo` was stopped after its filesystem fell below 1% free and returned HTTP 507 during the real attachment fixture. Its 50 files were copied with matching hashes to volatile `/dev/shm/cobalt-wiki-test-silo-20260922`; `cobalt-test-silo-ram` now serves ports `29000/29001`. Preserve the disk store. When disk free space exceeds Silo's reserve, stop the RAM service, copy the state back, then retire the RAM directory. Protected preview runtime files are `/home/osso/.local/share/cobalt-wiki/integration/preview.env`, `preview-deepwell.toml`, and `preview-nginx/nginx.conf`. They remain outside git and must not be printed. The local site is `6000011` (`cobalt-company`) with five archived homepage/navigation/theme-source fixtures and locally copied test-site roles.

Local observations (2026-09-22): authenticated homepage `200`, unauthenticated homepage `401`, theme CSS `200`. Browser hydration attaches the theme stylesheet and produces a black body background. The initial browser fixture showed one unresolved module and four unloaded images; attachments and form fixtures are absent. Screenshot: protected `integration/local-preview.png`. Local include expansion is committed, not deployed. Independent proof is `6/6` native cases at `df3547f` including `cd60c83`, plus FTML empty-argument `5/5`; paths are recorded below. It expands only same-site targets that pass anonymous `page:view`; foreign, viewer-private, and dynamically changed ACLs remain unsupported. Its size guard is post-expansion, not a memory cap. This is neither visual parity nor source-ACL/security proof: archived homepage/nav still require ListPages, SUO, and source-grammar work. Build the debug Deepwell binary before testing backend edits; `cargo check` alone does not update the executable. The current binary is not certified to match `HEAD`. Frontend changes hot-reload through Vite. Next proof: add minimal archived attachment/form fixtures, render actual archived includes locally, then browser-edit a hydrated form, save, reload, and verify the stored value.

## Local full-preview media routing

Verified 2026-09-23 by sanitized authoritative browser evidence at `/home/osso/.local/share/cobalt-wiki/local-full/review/missing-images-correction-verification.json`. The full preview is `127.0.0.1:3090` (Framerail `5174`, Deepwell `2749`, database `cobalt_local_full`). Missing images had two local causes: the database had no attachments and the gateway omitted `ALL` file routing, so image URLs returned HTML with status `200`. The earlier comparison also used `home:start`; the public root is `home:_public` (metadata ID `1312457213`), now the local default.

The local nginx configuration now mirrors the production file-route regex from `install/nixos/poc-gateway.nix`, retains Basic authentication, no-index, and trusted-site headers, and routes to WWS at `127.0.0.1:3470`. WWS started with its actual `S3_FILES_BUCKET` and `S3_TEXT_BLOCKS_BUCKET` contract; `.env.example`'s old `S3_BUCKET` and `ADDRESS` names are not the runtime contract. The source was unchanged; `priority-images-proof.json` established rerender proof. The correction verifier passed: authenticated `home:_public` loaded `2/2` visible images and `2/2` CSS backgrounds, with `4/4` same-origin assets returning `200`, correct MIME, decodable bytes, and matching manifest SHA-256; `home:start` loaded `19/19`, `3/3`, and `22/22` respectively. It found no failed image or asset URLs; unauthenticated access remained `401` and no-index remained present.

This is not full visual parity. The full import remains incomplete; do not infer full counts, start another importer, or deploy. See [same-site media routing](../../specs/cobalt-media-routing.md) for the rendering contract.

## Local navigation/sidebar correction

Verified 2026-09-23 at canonical revision `1310687`. The protected source baseline `/home/osso/.local/share/cobalt-wiki/local-full/review/layout-comparison-before.json`, at `1440×1000`, records a `24px` top navigation at `y=106` and no `#side-bar`; the earlier local comparison measured `182px` at `y=-52` and exposed a sidebar.

`41cf1af` nests FTML sublists in their parent list items, `eda5742` omits sidebar markup when the configured sidebar is empty while retaining enabled sidebars, `b4b25d4` persists `top_bar_page` and `side_bar_page` through `site_update`, and `1310687` scopes Sigma typography to its layout. The local full-preview setting `side_bar_page=''` reads back persisted; the two local homepages were rerendered without source or current-revision changes.

Browser proof `/tmp/claude/cobalt-navigation-browser-green.log` passed `1/1`: seven top-level tabs in one row, top-bar height `24px` at `y=106`, visible hover dropdown, and no sidebar. Independent verification remains pending. Automatic invalidation for navigation-setting changes is not implemented: only the two current local homepages were rerendered, so other cached pages can retain old navigation until refreshed.

## Local who-we-are paragraph proof

Verified 2026-09-23 at `c0d79e1`, following paragraph/comment rendering commit `2ffcebe`. The imported `who-we-are` source remains unchanged: 4,758 bytes, SHA-256 `81faa6b5a13d43bd59177409909c4c650df86550cd60601f7db36d54da260bbe`; its current revision also remains unchanged. Only that page was rerendered with local Deepwell `2749`.

The actual source document uses XHTML 1.0 Transitional. An otherwise identical standalone HTML probe measured a 5 px caption gap in XHTML Transitional and 13 px in HTML5. `c0d79e1` therefore selects the legacy doctype only for successful Wikidot content-root and slug documents; native, error, and special responses remain HTML5. Its focused response suite passed `4/4`.

At current code, `/tmp/claude/cobalt-who-we-are-browser-green.log` is green: `who-we-are` returns `200`; visible-text SHA matches source; all three images match source MIME/SHA and exact positions; at 1440×1000 the first caption is y=670, ImageBox height is 289 px, and article height is 1,605 px; the sidebar is absent; no JavaScript errors occurred. This is a bounded local browser result, not full-site visual parity/readiness. Independent final verification remains pending. The durable rendering contract is [paragraph formatting](../../specs/cobalt-paragraph-formatting.md); proposed Meilisearch/search remains a separate decision with no implementation.

## Acquisition and proof artifacts

The source API remains disabled with its original settings. No API key is required or requested. Canonical names were independently enumerated; colon-to-underscore forward mapping matches archive keys without gaps or collisions. Never invert underscore substitution to infer names.

Private evidence lives under `/home/osso/.local/share/cobalt-wiki/`: immutable import plan, complete importer log, `page-db-reconciliation.jsonl`, `file-db-reconciliation.jsonl`, parser corpus results, browser capture, and sanitized local-full media verification. Credentials and original source content are not tracked.

The FTML failure was reproduced with both the exact archive member and a public unmatched-`))` fixture. Correcting closing-token dispatch resolved it; logging suppression and blind mutation retries were reverted. Native DB regression, Rust fmt/check, and the 6,092-source parser corpus passed. Original bytes were not rewritten.

Local include proof at `df3547f` including `cd60c83`: native `6/6` (`/tmp/claude/cobalt-includes-verify-test.log`), fmt exit `0` (`/tmp/claude/cobalt-includes-verify-fmt-latest.log`), check exit `0` (`/tmp/claude/cobalt-includes-verify-check-latest.log`), and implementation FTML empty-argument `5/5`. No deployment or homepage visual fix follows from this proof.

Attachment invalidation local proof: `b007c0b` changes first file revision invalidation from page displacement to page edit. Valid RED queued one ordinary-link-dependent rerender after attachment creation; GREEN queued zero (`/tmp/claude/cobalt-file-invalidation-green.log`). `4fa4365` adds a dedicated-empty-Redis `--ignored` fixture: it retains site-navigation fanout when the attachment owner is the navigation page. Its final `1/1` isolated proof is `/tmp/claude/cobalt-file-invalidation-isolated-green.log`; independent verification remains pending. Nothing is deployed.

`292d7ec` adds a pure duplicate-candidate planner. It preserves all non-rerender, malformed, future, received, foreign-site, or non-byte-identical records. It is not a deletion tool and does not prove semantic redundancy.

Production queue observations, September 22, 2026: at 21:32 UTC, depth `4,353,956`, Valkey used memory `1,490,998,800` bytes and RSS `1,529,503,744` bytes. A later atomic read found depth `4,350,786`, `totalsent=4,548,267`, `totalrecv=197,519`, and five head jobs that were unreceived depth-2 navigation rerenders. These are observations, not an identified producer or recovery proof. User authorized a Cobalt-preview-only maintenance window of at most 10 minutes after the attachment fix is independently verified: back up, inspect, and remove only proven-safe byte-identical pending rerender duplicates; preserve other jobs, page/file data, and rollback evidence. Maintenance has not started.

## Imported source history

Protected homepage-pilot artifacts show a local-only import into site `6000011`: 240 records inserted, with the current revision, current source, and compiled output unchanged. Readback covered 240 bodies and metadata across seven cursor pages; an idempotent replay inserted zero records. `bd9d88f` adds local UI actions, and browser proof at `c7859d0` reads all 240 imported rows across five UI pages, views revision 0 source with its protected hash, preserves display-decoding provenance, offers no imported rollback, and reloads the current page (`/tmp/claude/cobalt-history-ui-green.log`). No production history was imported. Historical bodies remain display-decoded rather than byte-exact where source rendering converted whitespace; raw source responses retain provenance, unknown historical title/slug/tags remain null, and the current archived source stays byte-exact. Revision 239's decoder, import payload, and local API body match the archived 2,473-byte SHA-256 `eb0478369cf1cae46acb85b994c77162e3116ce7694ca7c5399ae26e46890beb`; an older unequal artifact predates decoder correction. The backend uses trusted request identity (`9c4ff20`) for history visibility rather than a JSON `user_id` override.

This is partial local proof only. The final integrated history verification is not established: `/tmp/claude/cobalt-imported-history-final-scoped-tests.log` exited `1` abnormally. History UI access remains coordinated with the rendering owner; site-wide authorized acquisition, source-backed attribution/metadata transitions, production schema/API deployment, and production import remain open.

Historical note: subagents were disabled for an earlier final-work phase. Do not treat that as a current workflow restriction.
