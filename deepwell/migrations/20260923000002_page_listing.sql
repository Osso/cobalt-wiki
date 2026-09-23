-- Which ListPages/CountPages selections a page body renders, so a page change
-- rerenders exactly the listings it could appear in (or disappear from).
CREATE TABLE page_listing (
    page_id BIGINT NOT NULL REFERENCES page(page_id),
    selection JSONB NOT NULL
);

CREATE INDEX page_listing_page_id_idx ON page_listing (page_id);
