-- Wikidot's site-wide revision list (its SiteChanges module): one row per
-- listed revision, as acquired by tools/cobalt_migration/wikidot_changes.py.
CREATE TABLE wikidot_site_change (
    site_id BIGINT NOT NULL REFERENCES site(site_id),
    page_slug TEXT NOT NULL,
    page_title TEXT NOT NULL,
    revision_number INTEGER NOT NULL,
    flags TEXT NOT NULL,
    changed_at TIMESTAMP WITH TIME ZONE NOT NULL,
    user_slug TEXT,
    user_name TEXT,
    source_user_id BIGINT,
    comments TEXT NOT NULL,
    PRIMARY KEY (site_id, page_slug, revision_number, changed_at)
);

CREATE INDEX wikidot_site_change_recent_idx ON wikidot_site_change (site_id, changed_at DESC);
