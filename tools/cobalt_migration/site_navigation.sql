\set ON_ERROR_STOP on
BEGIN;
-- Wikidot's theme for this site has no side bar: no page shows #side-bar
-- (checked 2026-09-24 on home:start, roster, who-we-are, writing, character,
-- player, arc and system pages), although nav:side exists. An empty
-- side_bar_page disables it; the site default was nav:side.
UPDATE site SET side_bar_page = '' WHERE site_id = 6000000;
-- Stored revisions keep the side bar they were rendered with; drop it.
UPDATE page_revision SET compiled_side_bar_html_hash = NULL
WHERE site_id = 6000000 AND compiled_side_bar_html_hash IS NOT NULL;
COMMIT;
