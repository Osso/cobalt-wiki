-- Shared draft per site and canonical page slug; not a published page revision.
CREATE TABLE page_draft (
    site_id BIGINT NOT NULL REFERENCES site(site_id),
    slug TEXT NOT NULL,
    title TEXT NOT NULL,
    wikitext TEXT NOT NULL,
    saved_by_user_id BIGINT REFERENCES known_user(user_id),
    updated_at TIMESTAMPTZ NOT NULL DEFAULT now(),
    PRIMARY KEY (site_id, slug)
);
