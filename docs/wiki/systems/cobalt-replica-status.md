# Cobalt replica status

Verified: 2026-09-22. This records evidence, not authorization to expose source-private content.

## Coverage

| Capability | Proven | Remaining |
|---|---|---|
| [Full POC import](../../specs/cobalt-poc-import.md) | 6,092 active pages and 1,471 attachments; importer exited successfully after byte/hash readback. Independent SQL comparison found no page name/title/tag/size/SHA-256 mismatches, missing/extra records, or wrong migration attribution. Attachment ownership/name/size inventory also matches. | Source authorship, creation dates, full revision/forum history and account mapping |
| [Source theme](../../specs/cobalt-poc-theme.md) | Archived CSS/font served behind authentication; source-image URLs resolve locally and three theme images match archive hashes over public HTTPS. Browser confirms source body/title fonts, black background and 1,000px container. | Full visual/layout parity and dynamic content rendering |
| [Native runtime](../../specs/cobalt-native-runtime.md) and [gateway](../../specs/cobalt-poc-gateway.md) | Native NixOS deployment, public HTTPS, loopback services, protected runtime secrets, Basic authentication, no-index headers, successful Sakuin readiness checks | Full source page/file ACL parity; forged invalid application-session cookies still cause an error |
| [Metadata acquisition](../../specs/cobalt-page-metadata.md) | Listing completed 277/277 pages; metadata finished with 6,090 accepted records, one denied and one redirect | Complete ACL/history/creator acquisition; neither unresolved record is fabricated |
| [Forms](../../specs/cobalt-data-form-schema.md) and rendering | Whole-record schema/value handling, authorized form edits, native DB regression coverage; all archived sources parse/render without panic after the FTML closing-delimiter correction | Hydrated form save/reload, includes/ListPages/form token parity, real link-title resolution; current pages still show upstream TODO placeholders |

## Access boundary

`cobalt-company.sakuin.org` is publicly reachable over HTTPS without a client tunnel. The existing Sakuin Cloudflare Tunnel connects to the loopback gateway. Every visitor still needs the shared POC login. Public tests cover page, CSS, file, download, robots and `.well-known` authentication challenges with `X-Robots-Tag: noindex, nofollow, noarchive`.

WWS does not implement complete per-page authorization. The shared gate grants its holders access to the whole POC; it must remain until source permission parity is proven and removal is authorized. Source creator identities remain unacquired, so target author/time records explicitly describe the technical migration, not original authorship.

## Acquisition and proof artifacts

The source API remains disabled with its original settings. No API key is required or requested. Canonical names were independently enumerated; colon-to-underscore forward mapping matches archive keys without gaps or collisions. Never invert underscore substitution to infer names.

Private evidence lives under `/home/osso/.local/share/cobalt-wiki/`: immutable import plan, complete importer log, `page-db-reconciliation.jsonl`, `file-db-reconciliation.jsonl`, parser corpus results, and browser capture. Credentials and original source content are not tracked.

The FTML failure was reproduced with both the exact archive member and a public unmatched-`))` fixture. Correcting closing-token dispatch resolved it; logging suppression and blind mutation retries were reverted. Native DB regression, Rust fmt/check, and the 6,092-source parser corpus passed. Original bytes were not rewritten.

Subagents were disabled for the final work. Final database, HTTP and browser checks were performed directly, not claimed as an independent-agent review.
