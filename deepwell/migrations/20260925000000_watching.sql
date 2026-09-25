-- Local subscriptions only; migration does not enroll users or backfill events.
CREATE TABLE watch_subscription (
    subscription_id BIGSERIAL PRIMARY KEY,
    user_id BIGINT NOT NULL REFERENCES "user"(user_id),
    site_id BIGINT NOT NULL REFERENCES site(site_id),
    category_id BIGINT REFERENCES page_category(category_id),
    page_id BIGINT REFERENCES page(page_id),
    created_at TIMESTAMP WITH TIME ZONE NOT NULL DEFAULT now(),
    CHECK (category_id IS NULL OR page_id IS NULL)
);

CREATE UNIQUE INDEX watch_subscription_site_unique
    ON watch_subscription (user_id, site_id)
    WHERE category_id IS NULL AND page_id IS NULL;
CREATE UNIQUE INDEX watch_subscription_category_unique
    ON watch_subscription (user_id, site_id, category_id)
    WHERE category_id IS NOT NULL;
CREATE UNIQUE INDEX watch_subscription_page_unique
    ON watch_subscription (user_id, site_id, page_id)
    WHERE page_id IS NOT NULL;

CREATE TABLE watch_preferences (
    user_id BIGINT PRIMARY KEY REFERENCES "user"(user_id),
    email_enabled BOOLEAN NOT NULL DEFAULT false,
    auto_watch BOOLEAN NOT NULL DEFAULT false
);

CREATE TABLE watch_event (
    event_id BIGSERIAL PRIMARY KEY,
    site_id BIGINT NOT NULL REFERENCES site(site_id),
    page_id BIGINT NOT NULL REFERENCES page(page_id),
    previous_revision_id BIGINT REFERENCES page_revision(revision_id),
    new_revision_id BIGINT NOT NULL UNIQUE REFERENCES page_revision(revision_id),
    actor_user_id BIGINT NOT NULL REFERENCES known_user(user_id),
    created_at TIMESTAMP WITH TIME ZONE NOT NULL DEFAULT now()
);

CREATE TABLE watch_notification (
    event_id BIGINT NOT NULL REFERENCES watch_event(event_id),
    user_id BIGINT NOT NULL REFERENCES "user"(user_id),
    activity_ready BOOLEAN NOT NULL DEFAULT false,
    processed_at TIMESTAMP WITH TIME ZONE,
    email_status TEXT NOT NULL DEFAULT 'disabled'
        CHECK (email_status IN ('disabled', 'pending', 'attempted', 'sent', 'failed')),
    unsubscribe_token_hash BYTEA UNIQUE,
    last_error TEXT,
    PRIMARY KEY (event_id, user_id)
);
