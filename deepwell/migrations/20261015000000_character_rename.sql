-- A character rename runs as one background job; this row reports its outcome.
-- Progress while running lives in Redis (uncommitted page edits are invisible).
CREATE TABLE character_rename (
    operation_id BIGSERIAL PRIMARY KEY,
    site_id BIGINT NOT NULL REFERENCES site(site_id),
    page_id BIGINT NOT NULL REFERENCES page(page_id),
    actor_user_id BIGINT NOT NULL REFERENCES known_user(user_id),
    old_name TEXT NOT NULL,
    new_name TEXT NOT NULL,
    new_slug TEXT NOT NULL,
    preview_token TEXT NOT NULL,
    ip_address TEXT NOT NULL,
    total INTEGER NOT NULL,
    status TEXT NOT NULL DEFAULT 'queued'
        CHECK (status IN ('queued', 'running', 'done', 'failed')),
    error TEXT,
    created_at TIMESTAMPTZ NOT NULL DEFAULT now(),
    finished_at TIMESTAMPTZ
);
