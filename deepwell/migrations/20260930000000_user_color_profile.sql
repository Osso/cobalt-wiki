ALTER TABLE "user"
    ADD COLUMN color_profile TEXT NOT NULL DEFAULT 'current'
    CHECK (color_profile IN ('classic', 'current'));
