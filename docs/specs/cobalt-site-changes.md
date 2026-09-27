# Cobalt site changes

Site Changes renders the imported Wikidot `wikidot_site_change` feed in the current wiki. It targets module-behavior parity using the Files-table visual direction established by `97261dde1`, rather than copying Wikidot’s legacy styling. [Replica status](../wiki/systems/cobalt-replica-status.md) records implementation evidence and rollout limits.

## What it must do

### Feed and filtering

- [x] Preserve revision metadata without rewriting page source or history; report feed coverage separately from rendering behavior.
- [x] Preserve module category selection: `All` wins over selected change filters; selected non-`All` filters use OR semantics.
- [x] Preserve page sizes 10, 20, 50, 100, and 200 (default 20), reset to URL page 1 on filter change, and retain pager links.

### Rendering and time

- [x] Render a semantic five-column table: Page, Changes, Revision, Changed, and Author.
- [x] Render change titles and comments; display N, S, T, R, A, M, and F flags; render revision 0 as `(new)`; render a slugless imported author as escaped plain text.
- [x] Emit ISO-second UTC server times in `datetime` plus `data-timestamp`, then localize them in the browser and expose relative time on hover and keyboard focus. The replica’s 20 hidden `.odate` spans are not source behavior; original Wikidot dates render inline.
- [x] Preserve the Files UI from `97261dde1` exactly while meeting the Site Changes module behavior contract.

## How it works

- [Replica status](../wiki/systems/cobalt-replica-status.md) records source observations, implementation evidence, and rollout limits.

## Implementation inventory

- `36f9007b8` — renderer: semantic five-column output, comments, flags, ISO-second UTC values, and plain slugless authors.
- `fe800e900` — default-view cache correction: default pages read the imported feed too; real-DB update regression covered.
- `603ec2ce8` — frontend localization and Site Changes controls; 9/9 frontend checks pass.
- `84d897cff` — waits for semantically localized dates and fixes type warnings, replacing the earlier Vite-overlay and SSR-before-hydration controls race evidence.
- `131f5bae21b1` — production release installed by `install/dev-deploy.sh all`; deployment preceded the approved import work.
- `wikidot_site_change` — imported source-feed boundary.

## Tests asserting this spec

- `deepwell/tests/page_site_changes.rs` — 3/3 targeted real-PostgreSQL tests: newest-first paging, categories/flags/page sizes/pager URLs, valid escaped table output, ISO/UTC serialization, and slugless author rendering (`/tmp/claude/cobalt-sitechanges-targeted-pass.log`).
- Frontend Site Changes checks — 9/9 at `603ec2ce8`.
- `framerail/tests/local/site-changes.mjs` — read-only browser proof 1/1: 20 visible Chicago-local dates with seconds and hover/focus relative time; writing/A filters across two disjoint 10-row pages; `All` reset; page sizes yield 10/20/50/99/99 (`/tmp/claude/cobalt-sitechanges-browser-ready.log`). Screenshot inspected: `/tmp/claude/cobalt-sitechanges-local.png`.

## Coverage matrix

| Area | Status | Proof / limit |
|---|---|---|
| Renderer | Green | `36f9007b8`: semantic five-column table, comments, flags, ISO-second UTC, and plain slugless author. |
| Default view | Green | `fe800e900` reads the feed for default pages too; covered by a real-DB update regression. |
| Backend | Green | 3/3 targeted tests; independent format/check/vendor-format gate is `PASS_WITH_INHERITED_READABILITY_DEBT` (`/tmp/claude/cobalt-sitechanges-backend-gate.json`). |
| Frontend | Green, bounded | `603ec2ce8` 9/9; `84d897cff` supersedes the initial race evidence. Final frontend follow-up passes: Svelte 0 errors/0 warnings and scoped format/lint/style checks clean (`/tmp/claude/cobalt-sitechanges-final-followup.json`). |
| Local runtime | Green | Root deploy exited 0; running and built hashes match. Local only. |
| Browser | Green | Read-only local browser 1/1 as described above. |
| Production rollout | Green, bounded | `install/dev-deploy.sh all` released `131f5bae21b1` with exit 0. Independent read-only production gate PASS: Deepwell binary and all 553 Framerail build files match, services are active, public Site Changes returns 20 default rows and two writing/`A` pages with zero blocked writes or page errors (`/tmp/claude/cobalt-sitechanges-production-gate.json`). WWS restarted and completed healthy startup; evidence does not prove its cause. |

## Source evidence and data limits

- Live read-only GET confirms revision-type tags and four `A`-flag rows (`/tmp/claude/cobalt-sitechanges-live-tags-proof.json`). The retained old repository has six flags, not the current live seven; it is not definitive current-source evidence.
- Imported feed scope is only `wikidot_site_change`: 99 local rows, latest `2026-09-24T16:45:15Z`, with 43 writing-tagged rows (`/tmp/claude/cobalt-sitechanges-feed-scope.json`). It does not establish freshness against live Wikidot on 2026-09-27.
- Source page contents and history are unchanged. The production release and its read-only gate did not import, rewrite, merge, or freshly acquire source data; no native-feed merging or privacy contract is claimed.
- Read-only dry URL inventory remains unchanged: 269 old-host tokens in 196 pages; 260 mapped targets across 189 pages; seven unconfirmed and two non-page paths. It is not syntax parsed or apply-ready. No migration, import rewrite policy, or rewrite was implemented.

## Known gaps (current cycle)

- [x] Final frontend follow-up passes; inherited backend readability debt is recorded separately and was not expanded by this change.

## Out of scope

- Native-feed merging, fresh source acquisition, source freshness guarantees, privacy-policy inference, and rewriting current or archived content/history.
