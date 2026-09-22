# Cobalt replica status

Verified: 2026-09-22. This records evidence, not authorization to expose source-private content.

## Coverage

| Capability | Proven | Remaining |
|---|---|---|
| [Full POC import](../../specs/cobalt-poc-import.md) | 6,092 active pages and 1,471 attachments; importer exited successfully after byte/hash readback. Independent SQL comparison found no page name/title/tag/size/SHA-256 mismatches, missing/extra records, or wrong migration attribution. Attachment ownership/name/size inventory also matches. | Source authorship, creation dates, full revision/forum history and account mapping |
| [Source theme](../../specs/cobalt-poc-theme.md) | Archived CSS/font served behind authentication; source-image URLs resolve locally and three theme images match archive hashes over public HTTPS. Browser confirms source body/title fonts, black background and 1,000px container. | Full visual/layout parity and dynamic content rendering |
| [Native runtime](../../specs/cobalt-native-runtime.md) and [gateway](../../specs/cobalt-poc-gateway.md) | Native NixOS deployment, public HTTPS, loopback services, protected runtime secrets, Basic authentication, no-index headers, successful Sakuin readiness checks | Full source page/file ACL parity; forged invalid application-session cookies still cause an error |
| [Metadata acquisition](../../specs/cobalt-page-metadata.md) | Listing completed 277/277 pages; metadata finished with 6,090 accepted records, one denied and one redirect | Complete ACL/history/creator acquisition; neither unresolved record is fabricated |
| [Forms](../../specs/cobalt-data-form-schema.md) and rendering | Whole-record schema/value handling, authorized form edits, native DB regressions; all archived sources parse/render without panic. [Automatic link titles](../../specs/cobalt-link-titles.md) now use actual target revisions; four affected cached pages refreshed without new revisions | Hydrated form save/reload, includes/ListPages/form token parity, inline-file domain routing and multi-colon reference normalization |

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
| Storage | Silo `127.0.0.1:29000` | reuse `cobalt-test-silo`; do not restart for preview work |
| Queue/cache | Valkey `127.0.0.1:26381`, database `1` | preview-only database; test harness uses database `0` |

Reuse the existing user services `cobalt-test-postgres`, `cobalt-test-silo`, and `cobalt-test-valkey`; do not start parallel copies. Protected preview runtime files are `/home/osso/.local/share/cobalt-wiki/integration/preview.env`, `preview-deepwell.toml`, and `preview-nginx/nginx.conf`. They remain outside git and must not be printed. The local site is `6000011` (`cobalt-company`) with five archived homepage/navigation/theme-source fixtures and locally copied test-site roles.

Local observations (2026-09-22): authenticated homepage `200`, unauthenticated homepage `401`, theme CSS `200`. Browser hydration attaches the theme stylesheet and produces a black body background. The initial browser fixture showed one unresolved module and four unloaded images; attachments and form fixtures are absent. Screenshot: protected `integration/local-preview.png`. Navigation was subsequently configured directly in the local fixture database because `site_update` does not persist navigation fields, then the local homepage was rerendered. These observations do not establish visual or form parity. Build the debug Deepwell binary before testing backend edits; `cargo check` alone does not update the executable. The current binary is not certified to match `HEAD`. Frontend changes hot-reload through Vite. Next proof: add minimal archived attachment/form fixtures, then browser-edit a hydrated form, save, reload, and verify the stored value.

## Acquisition and proof artifacts

The source API remains disabled with its original settings. No API key is required or requested. Canonical names were independently enumerated; colon-to-underscore forward mapping matches archive keys without gaps or collisions. Never invert underscore substitution to infer names.

Private evidence lives under `/home/osso/.local/share/cobalt-wiki/`: immutable import plan, complete importer log, `page-db-reconciliation.jsonl`, `file-db-reconciliation.jsonl`, parser corpus results, and browser capture. Credentials and original source content are not tracked.

The FTML failure was reproduced with both the exact archive member and a public unmatched-`))` fixture. Correcting closing-token dispatch resolved it; logging suppression and blind mutation retries were reverted. Native DB regression, Rust fmt/check, and the 6,092-source parser corpus passed. Original bytes were not rewritten.

Historical note: subagents were disabled for an earlier final-work phase. Do not treat that as a current workflow restriction.
