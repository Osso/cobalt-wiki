# Cobalt site changes

Site Changes will render the imported Wikidot `wikidot_site_change` feed in the current wiki. It targets full module-behavior parity while using the modern Files-table visual direction, not literal Wikidot styling. Deployment state is tracked in the [replica status](../wiki/systems/cobalt-replica-status.md).

## What it must do

### Feed and filtering

- [ ] Read only imported `wikidot_site_change` rows; native-feed merging, freshness, and privacy semantics are not implied.
- [ ] Preserve existing category selection; `All` wins over selected change filters, and other selected filters use OR semantics.
- [ ] Preserve page sizes 10, 20, 50, 100, and 200 (default 20), reset to URL page 1 when filters change, and retain pager links.

### Rendering

- [ ] Render a semantic five-column table: Page, Changes, Revision, Changed, and Author.
- [ ] Render each change title and comments; display N, S, T, R, A, M, and F flags; render revision 0 as New; display authors only when their imported names are known strings.
- [ ] Emit UTC server times with `datetime` ISO data and `data-epoch`; localize in the client timezone and expose relative time on hover and keyboard focus.
- [ ] Match module behavior, while styling the table after the current Files UI at `97261dde1`, rather than reproducing Wikidot's legacy appearance.

## How it works

- [Replica status](../wiki/systems/cobalt-replica-status.md) records source evidence, implementation status, and rollout limits.

## Implementation inventory

- Pending — no Site Changes-specific renderer, frontend component, or tests are introduced by this documentation-only change.
- `wikidot_site_change` — imported source-feed boundary; source acquisition remains separate from rendering.

## Tests asserting this spec

- None yet.

## Coverage matrix

| Area | Status | Proof / limit |
|---|---|---|
| Main semantic markup | Missing | Pending implementation. |
| Frontend | Partial evidence | `603ec2ce89/9`; not browser proof. |
| Backend | RED expected | `38914d968` confirms expected markup is absent. |
| Browser | Missing | Pending. |
| Production rollout | Not authorized | No production rollout for this feature. |

## Source evidence and data limits

- Public source observation found all 20 sampled `odate` elements `display:none`; inline `.localdatetime` showed seconds and an “ago” relative value on hover. The target time contract above is explicit; this observation does not establish source freshness or privacy behavior.
- The legacy repository lacks `A` tags while the live source has them. Treat that as a source-parity gap until live behavior is separately captured.
- Read-only candidate inventory (`/tmp/claude/cobalt-legacy-page-link-dry-run.json`, mode `0600`) covers 6,117 current local pages: 196 pages contain 269 old-host URL tokens; 260 targets are confirmed across 189 pages; seven are not in plan and two are non-page paths. The regex inventory does not parse link contexts and is not apply-ready; it made no mutations.
- A future current-source import may transform qualifying links in memory before its current payload. It must not rewrite archived/history revisions. Existing local pages require latest-revision compare-and-swap protection and must not overwrite newer local edits. No rewrite policy or apply is authorized.

## Known gaps (current cycle)

- [ ] Implement the main renderer markup and backend/frontend behavior.
- [ ] Add backend and browser assertions for the table, filters, pagination, flags, author visibility, and localized time behavior.
- [ ] Capture source behavior for live `A` flags without treating legacy repository output as definitive.

## Out of scope

- Native feed merging, source-feed acquisition/freshness guarantees, privacy-policy inference, historical revision rewriting, production rollout, and applying legacy-link rewrites.
