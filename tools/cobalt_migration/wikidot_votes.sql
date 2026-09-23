\set ON_ERROR_STOP on
BEGIN;
-- fakecat:testing's only vote on Wikidot (WhoRatedPageModule, 2026-09-23): OzmaAsimov +1.
-- Wikidot does not show when it was cast; created_at is the import time.
INSERT INTO page_vote (from_wikidot, page_id, user_id, value)
SELECT true, p.page_id, 7444794, 1 FROM page p
WHERE p.site_id = 6000000 AND p.slug = 'fakecat:testing' AND p.deleted_at IS NULL
ON CONFLICT DO NOTHING;
COMMIT;
