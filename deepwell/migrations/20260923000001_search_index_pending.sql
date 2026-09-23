-- The generation changes on every enqueue, including repeated edits in one transaction.
-- A worker only acknowledges the generation it actually sent to Meilisearch.
CREATE TABLE search_index_pending (
    page_id BIGINT PRIMARY KEY REFERENCES page(page_id) ON DELETE CASCADE,
    generation BIGSERIAL NOT NULL
);

-- Initial backfill is durable; deleted pages have no preexisting index document.
INSERT INTO search_index_pending (page_id)
SELECT page_id FROM page WHERE deleted_at IS NULL;
