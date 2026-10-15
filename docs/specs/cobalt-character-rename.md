# Cobalt character rename

A character's short name is the page name in `character:<name>` or `bgc:<name>`. The site uses it as the character's tag, its `re:<name>` mention tag, a token in writing/arc/chain name lists, the `RelationshipBox` `pagename`, link targets, and the portrait file `icons/icon_<name>.jpg`. Renaming changes all of these together. The display name (page title, form `name`) does not change.

Decisions (user, 2026-10-15): anyone who can edit the character page may rename it; it runs as a background job with progress; there is no dedicated undo (rename back instead).

## What it must do

- [x] `character_rename_preview {page_id,new_name}` returns `{old_name,new_name,old_slug,new_slug,preview_token,icon,count,pages}` for a live `character:`/`bgc:` page (not `_template`). The new name is trimmed and lowercased and must be lowercase letters, digits and single hyphens, different from the current name.
- [x] Preview and start require Page/Edit (and lock bypass) on the character page, on every affected page, and on `icons` when the icon is renamed. Denial on the character page is `PermissionDenied` 3106; on another page, `TagPageNotEditable` 4306.
- [x] The name must be unused: an existing `character:<new>`/`bgc:<new>` is `PageSlugExists` 2104; any live page tagged `<new>` or `re:<new>` is `CharacterNameInUse` 4307; an existing `icons/icon_<new>.jpg` is `FileExists` 2105.
- [x] Affected pages are live pages whose change set is non-empty: tags `<old>`→`<new>` and `re:<old>`→`re:<new>`; writing/arc/chain `characters`/`references` whole tokens (same rule as [tag rename](cobalt-tag-admin.md)); `pagename=<old>` values; and `[[[character:<old>…]]]`/`[[[bgc:<old>…]]]` link targets. Prose, other words containing the name, and other fields stay unchanged.
- [x] `character_rename_start {page_id,new_name,preview_token,ip_address}` succeeds only if the recomputed preview token matches (`TagPreviewStale` 4304 otherwise), records the operation, and queues the job; it returns `{operation_id}`.
- [x] The job re-checks the token under the site tag lock, then in one transaction moves the page to `<category>:<new>`, makes one ordinary revision per affected page (the character page gets its move revision plus one), and renames the icon file. Any failure rolls everything back and records the reason; nothing is retried. Changes after confirmation make the job fail stale.
- [x] `character_rename_status {operation_id}` returns `{operation_id,status,done,total,error,new_slug}` to the renaming user only; `status` is `queued`, `running`, `done`, or `failed`. `total` counts the move, each page edit, and the icon rename.
- [ ] The Rename character pane on character/bgc pages previews, confirms, shows progress, and opens the new page when done ([page actions](cobalt-page-actions.md)).
- [ ] Production rename verified end to end, including the worker transaction and progress polling.

## How it works

- `deepwell/src/services/character_rename.rs`: planning (candidate pages: tag overlap or the name as a word in the source), preview token over page IDs/revisions and the icon revision, job body `execute`, and `run_job`, which claims a queued row, runs `execute` in its own transaction, and records failure in a separate one.
- The job runs on the existing worker queue. It may outlast the queue's 30 s process time; a redelivered message finds the row already claimed and does nothing.
- Progress is a Redis counter (`character-rename:progress:<id>`, 24 h expiry), because uncommitted edits are invisible to status reads. The `character_rename` row holds the final outcome.
- The job holds the site tag lock for its whole run, so ordinary page edits and imports on the site wait until it finishes. At about 76 ms per real page, a character on 1,000 pages blocks edits for roughly 75 s.

## Implementation inventory

- `deepwell/src/services/character_rename.rs`, `deepwell/src/endpoints/character_rename.rs`, `deepwell/src/api.rs` — service and RPC registration.
- `deepwell/src/services/job/structs.rs`, `deepwell/src/services/job/worker.rs` — `Job::CharacterRename`.
- `deepwell/src/services/site_tag_admin.rs` — shared tag/name-list rename helpers.
- `deepwell/migrations/20261015000000_character_rename.sql` — operation table.
- `deepwell/src/error/error_type.rs` — `CharacterNameInUse` 4307.

## Tests asserting this spec

- `deepwell/tests/character_rename.rs` — registered preview/start/status plus the job body against the native DB, S3 and rendering: move, tags, writing/arc name lists, `pagename` and link rewrites with rendered icon/link, untouched prose page, icon rename, revision counts, progress `done`; slug/tag/icon/name/category conflicts; locked affected and character pages; stale preview at start and at job time. 5/5 (`/tmp/claude/character-rename-native-1.out`). With the reference rewrite and job stale check disabled, 3/5 fail (`/tmp/claude/character-rename-mutant.out`).
- `deepwell/src/services/character_rename.rs` unit tests — reference rewrite boundaries and new-name validation, 2/2.

## Known gaps (current cycle)

- `run_job` (claim, own transactions, failure recording) is not covered by the rollback-only test harness.
- A worker crash mid-job leaves the row `running`; nothing marks it failed.
- `[[include CharacterList | tags=…]]` terms and absolute `https://…/character:<name>` URLs are not rewritten (4 and 2 occurrences site-wide for the five most-referenced characters).
