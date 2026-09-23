\set ON_ERROR_STOP on
BEGIN;
-- Wikidot shows "Private content" to anonymous visitors in these categories
-- (checked 2026-09-23). A category with scoped page:view grants overrides _default.
INSERT INTO role_permission (role_id, site_id, resource_type, resource_category_id, action)
SELECT r.role_id, 6000000, 'page', c.category_id, 'view'
FROM page_category c
JOIN (VALUES
  ('admin', 'admin'), ('admin', 'root'),
  ('membersonly', 'member'), ('membersonly', 'moderator'), ('membersonly', 'admin'), ('membersonly', 'root'),
  ('nav', 'member'), ('nav', 'moderator'), ('nav', 'admin'), ('nav', 'root'),
  ('application', 'member'), ('application', 'moderator'), ('application', 'admin'), ('application', 'root'), ('application', 'page-author')
) AS g(category, role) ON g.category = c.slug
JOIN role r ON r.name = g.role AND r.site_id = 6000000 AND r.deleted_at IS NULL
WHERE c.site_id = 6000000
ON CONFLICT DO NOTHING;
COMMIT;
-- Then clear Deepwell's permission cache: valkey keys permission:site:6000000:*
