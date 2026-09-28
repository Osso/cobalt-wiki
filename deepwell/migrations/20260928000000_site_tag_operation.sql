CREATE TABLE site_tag_operation (
    operation_id BIGSERIAL PRIMARY KEY,
    site_id BIGINT NOT NULL REFERENCES site(site_id),
    actor_user_id BIGINT NOT NULL REFERENCES known_user(user_id),
    source_tag TEXT NOT NULL,
    target_tag TEXT,
    created_at TIMESTAMPTZ NOT NULL DEFAULT now(),
    reversed_at TIMESTAMPTZ
);

CREATE TABLE site_tag_operation_page (
    operation_id BIGINT NOT NULL REFERENCES site_tag_operation(operation_id),
    page_id BIGINT NOT NULL REFERENCES page(page_id),
    slug TEXT NOT NULL,
    title TEXT NOT NULL,
    before_tags TEXT[] NOT NULL,
    after_tags TEXT[] NOT NULL,
    resulting_revision_id BIGINT NOT NULL REFERENCES page_revision(revision_id),
    PRIMARY KEY (operation_id, page_id)
);
CREATE INDEX site_tag_operation_site_history ON site_tag_operation (site_id, operation_id DESC);
