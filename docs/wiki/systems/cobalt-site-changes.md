# Site Changes native feed

verified: 2026-10-14

Contract: [Site Changes spec](../../specs/cobalt-site-changes.md). Authority: [Sakuin wiki authority](../../../../sakuin/docs/cobalt-wiki-host.md#wiki-authority).

## Production diagnosis (read-only)

Deepwell `page_view` for site `6000000`, `system:recent-changes` returned source `[[module SiteChanges]]`. Its first rendered UTC timestamp was `2026-10-13T14:48:29Z`. PostgreSQL had 45,566 `wikidot_site_change` rows, with that same maximum timestamp.

Native `page_revision` rows were already committed: `bgc:bun` rev. 2 at `2026-10-14T18:46:37.231462Z`, `bgc:bael` rev. 2 at `18:46:20.271314Z`, and `bgc:audric` rev. 2 at `18:46:06.757380Z`, all with `changes={tags}` and `from_wikidot=false`. `bgc:bun` was absent from the rendered feed. Production response artifacts: `/tmp/cobalt-native-changes-production-view.json`, `/tmp/cobalt-native-changes-production.html`.

Native persistence worked. `fetch_site_changes` selected only `wikidot_site_change`, so existing view-time live rerendering could not show native events. Disabled sync is intentional, not a failed refresh mechanism.

## Read path

`deepwell/src/services/render/render_data.rs` reads imported history and non-Wikidot `page_revision` rows in one query, then filters, counts, sorts and pages the combined result. Native page links use current page slugs; titles/comments come from the revision, respecting hidden fields. Author identity joins the existing local/Wikidot user records. Imported history remains untouched; imported `page_revision` rows are excluded from the native branch.

No event duplication, scheduler, migration, backfill or write-path change is needed: existing native revisions become visible when Deepwell runs this query. Existing default-view live rerendering remains in place.

## Development proof and rollout limits

`deepwell/tests/page_site_changes_native.rs` creates a UUID-owned site, commits page-create/edit endpoint requests, and reads new transactions through `page_view`, including a view taken before editing. It checks persisted native tags and subsequent feed rows, native field flags/author, no-op edits, combined pagination with enough rows for page 2, and all 25 untouched imported history rows. Owned committed site is retired before assertions. Existing imported-only fixtures explicitly mark their feed-page revision imported.

Targeted RED: `cargo test --test page_site_changes_native -- --test-threads=1`, `/tmp/cobalt-native-changes-red.log`; persisted tag edit absent from a subsequent default feed view. Targeted GREEN: 4/4 at `1d7f94ee4`, `cargo test --test page_site_changes_native --test page_site_changes -- --test-threads=1`, `/tmp/cobalt-native-changes-green-final.log`. Fixture extraction at `e6675928a` reran only the changed native test (1/1), `/tmp/cobalt-native-changes-extracted-green.log`; unchanged imported tests retain their earlier proof. Ledger: `/tmp/cobalt-native-changes-proof.json`. Isolated PostgreSQL `25432/cobalt_test`, Valkey `26381/14`, RAM Silo `29000`; protected environment unchanged.

Main owns final gates and `deploy.sh deepwell` to Sakuin, including production read-only smoke proof. No frontend deployment is required by this change. No production mutation, deployment, push or restart occurred in this slice.

The existing migration/sync path also created some revisions with `from_wikidot=false`; this read path cannot retrospectively infer their authorship. It includes those stored rows rather than deleting history or guessing provenance. File-revision feed expansion and new per-page visibility policy are not covered by this slice. Native slug-change flags and hidden-field handling are not asserted by the new regression; author display is asserted, but native profile-link routing is not.
