# Cobalt rerender invalidation

When a page changes, Deepwell queues `rerender_page` jobs (Valkey rsmq queue `rsmq:job`) for the pages whose stored output depends on it. Sources: `deepwell/src/services/outdate.rs`, `deepwell/src/services/job/`, `PageRevisionService::rerender`. The queue incidents that shaped this contract are in [queue recovery](../wiki/systems/cobalt-queue-recovery.md).

## What it must do

- [x] A change (edit, create, delete, move, restore, attachment) queues every affected page at that moment: pages that include it (includes are recorded transitively), pages in its category when it is a `_template`, listing pages whose selection it matches ([ListPages](cobalt-list-pages.md)), linking pages on create/delete, and a navigation rerender of every page using it as a top or side bar. (Linking-page invalidation on create/delete is existing behavior without a dedicated test here.)
- [x] A queued page that is itself a navigation page also queues a navigation rerender of every page using that bar, at the same moment (bars are compiled into each page and are not recorded as dependencies).
- [x] Dependent jobs are `standalone` rerenders and queue nothing further; one change produces at most one job per affected page and type.
- [x] A job for a page and type that is already pending is not queued again. The marker (`job:rerender-pending:<page_id>:<type>`, 1 h expiry) is cleared when a worker starts the job, so a later change queues the page again.
- [x] A rerender caused by a renderer build change queues no dependents: a `full` rerender outdates dependents only when the page's stored output came from the current build and the new output differs (its source changed without an edit). A `full` rerender with unchanged output queues nothing.
- [x] `standalone` and view-time rerenders (stale `compiled_generator`) queue nothing.

## How it works

- [Queue recovery](../wiki/systems/cobalt-queue-recovery.md), [replica status](../wiki/systems/cobalt-replica-status.md).

## Implementation inventory

- `deepwell/src/services/outdate.rs` — finds dependents at change time; `outdate_pages` queues them and fans out navigation pages.
- `deepwell/src/services/job/service.rs` — `queue_rerender` with the pending marker; `start_rerender_job` clears it.
- `deepwell/src/services/job/worker.rs` — clears the marker, then runs `PageRevisionService::rerender`.
- `deepwell/src/services/page_revision/service.rs` — `rerender`: `full` outdates only on a same-build output change; edits call the outdater directly.

## Tests asserting this spec

- `deepwell/tests/page_rerender_fanout.rs` (dedicated Redis, `--ignored`): renderer-version sweep queues nothing; a nested include change queues each dependent once plus one navigation rerender per page, and draining queues nothing; pending jobs collapse until a worker starts them.
- `deepwell/tests/page_standalone_rerender.rs`: standalone and unchanged `full` rerenders queue nothing; `full` after an out-of-band source change queues dependents.
- `deepwell/tests/page_listing_invalidation.rs`, `deepwell/tests/file_attachment_invalidation.rs`: listing and attachment dependents.

## Known gaps (current cycle)

- [ ] Jobs are queued before the change's transaction commits (existing ordering). With deduplication, a second change that finds its job pending relies on that job reading after the second change commits; a job received within that commit window renders the older state.
- [ ] `rerender-skip` rules are inverted in code (`updated_recently!` is true when the page was *not* updated within the window), so they skip slow deep jobs and run fast loops. Jobs no longer queue jobs, so dependent jobs have depth 1 and production's rules (from depth 3) never apply; the built-in test default `(1, 100 ms)` would skip depth-1 jobs of pages not updated in the last 100 ms. Kept unchanged pending a decision to delete them.
- [ ] Navigation-page wikitext for bars is read by `get_latest_text_optional`, whose `IN (subquery ORDER BY …)` does not select the latest revision, so an edited navigation page can render an older revision into bars.

## Out of scope

- Navigation-setting changes (`site_update` of `top_bar_page`/`side_bar_page`) queue nothing; see [replica status](../wiki/systems/cobalt-replica-status.md).
