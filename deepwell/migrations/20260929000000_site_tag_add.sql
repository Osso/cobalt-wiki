ALTER TABLE site_tag_operation ALTER COLUMN source_tag DROP NOT NULL;
ALTER TABLE site_tag_operation ADD COLUMN kind TEXT;
UPDATE site_tag_operation SET kind = CASE WHEN target_tag IS NULL THEN 'delete' ELSE 'rename' END;
ALTER TABLE site_tag_operation ALTER COLUMN kind SET NOT NULL;
ALTER TABLE site_tag_operation ADD COLUMN filters JSONB;
ALTER TABLE site_tag_operation ADD CONSTRAINT site_tag_operation_kind_check CHECK (
    (kind = 'add' AND source_tag IS NULL AND target_tag IS NOT NULL AND filters IS NOT NULL)
    OR (kind IN ('rename', 'delete') AND source_tag IS NOT NULL AND filters IS NULL
        AND (kind = 'delete') = (target_tag IS NULL))
);
