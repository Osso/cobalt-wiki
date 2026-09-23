-- Source revision history is a read-only acquisition record, separate from
-- editable page revisions. NULL historical metadata means not acquired.
-- Raw HTML is retained because displayed source may change tabs to spaces.
ALTER TABLE page ADD CONSTRAINT page_id_site_id_unique UNIQUE (page_id, site_id);

CREATE TABLE imported_page_revision (
    site_id BIGINT NOT NULL REFERENCES site(site_id),
    page_id BIGINT NOT NULL,
    source_page_id BIGINT NOT NULL CHECK (source_page_id > 0),
    source_revision_id BIGINT NOT NULL CHECK (source_revision_id > 0),
    source_revision_number INTEGER NOT NULL CHECK (source_revision_number >= 0),
    source_author_id BIGINT CHECK (source_author_id > 0),
    source_created_at TIMESTAMP WITH TIME ZONE NOT NULL,
    source_comments TEXT NOT NULL,
    source_flags TEXT[] NOT NULL,
    source_title TEXT,
    source_slug TEXT,
    source_tags TEXT[],
    wikitext_hash BYTEA NOT NULL REFERENCES text(hash),
    raw_source_html TEXT NOT NULL,
    acquired_at TIMESTAMP WITH TIME ZONE NOT NULL,
    representation TEXT NOT NULL CHECK (representation = 'display-decoded-not-byte-exact'),

    PRIMARY KEY (site_id, page_id, source_revision_id),
    UNIQUE (site_id, page_id, source_revision_number),
    FOREIGN KEY (page_id, site_id) REFERENCES page(page_id, site_id)
);

COMMENT ON TABLE imported_page_revision IS
    'Read-only Wikidot source history; unknown historical metadata remains NULL. Does not replace the current native page revision.';
