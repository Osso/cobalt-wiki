-- Preserve existing-page origin so Create permission cannot expose orphaned drafts.
ALTER TABLE page_draft
    ADD COLUMN origin_page_id BIGINT REFERENCES page(page_id);
