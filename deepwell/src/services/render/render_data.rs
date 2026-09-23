use super::prelude::*;
use super::{BodyArguments, SiteChangesFilter};
use crate::models::page::Entity as Page;
use ftml::data::PageInfo;
use ftml::render::Handle;
use ftml::render::handle::{SiteChange, SiteChanges};
use ftml::tree::{
    DefinitionListItem, Element, LinkLabel, LinkLocation, ListItem, Module, SyntaxTree,
    Tab, Table,
};
use sea_orm::{DatabaseBackend, FromQueryResult, Statement, Value};
use std::collections::{BTreeMap, BTreeSet};

#[derive(Debug, FromQueryResult)]
struct PageTitle {
    site: String,
    page: String,
    title: String,
}

#[derive(Debug, FromQueryResult)]
struct TagWeight {
    tag: String,
    weight: i64,
}

#[derive(Debug, FromQueryResult)]
struct SiteChangeRow {
    page_slug: String,
    page_title: String,
    flags: String,
    changed_at: i64,
    revision_number: i32,
    user_slug: Option<String>,
    user_name: Option<String>,
    comments: String,
    total: i64,
}

#[derive(Debug, FromQueryResult)]
struct CategorySlug {
    slug: String,
}

#[derive(Debug, FromQueryResult)]
struct TaggedPage {
    slug: String,
    title: String,
}

/// What a parsed page needs from the database before rendering.
#[derive(Debug, Default)]
struct Wanted {
    pages: BTreeSet<(String, String)>,
    tag_cloud: bool,
    pages_by_tag: bool,
    site_changes: bool,
}

/// Fetch the data FTML renders from: titles of linked pages, plus site tags
/// for TagCloud, the pages carrying the URL's `tag` for PagesByTag and a page
/// of Wikidot's revision list for SiteChanges. The flag is set when the
/// output depends on every page's tags (TagCloud).
pub(super) async fn fetch_render_data(
    ctx: &ServiceContext<'_>,
    tree: &SyntaxTree<'_>,
    page_info: &PageInfo<'_>,
    body: Option<&BodyArguments>,
) -> Result<(Handle, bool)> {
    let site = &page_info.site;
    let tag = body.and_then(|body| body.tag.as_deref());
    let wanted = collect_wanted(tree, site);
    let page_titles = fetch_page_titles(ctx, wanted.pages).await?;
    let tag_weights = match wanted.tag_cloud {
        true => fetch_tag_weights(ctx, site).await?,
        false => Vec::new(),
    };
    let tagged_pages = match (wanted.pages_by_tag, tag) {
        (true, Some(tag)) => {
            Some((tag.to_owned(), fetch_tagged_pages(ctx, site, tag).await?))
        }
        _ => None,
    };
    let site_changes = match wanted.site_changes {
        true => {
            let fullname = match &page_info.category {
                Some(category) => format!("{category}:{}", page_info.page),
                None => page_info.page.to_string(),
            };
            let default = BodyArguments::default();
            let body = body.unwrap_or(&default);
            Some(
                fetch_site_changes(ctx, site, fullname, body.list_page, &body.changes)
                    .await?,
            )
        }
        false => None,
    };
    let handle = Handle {
        page_titles,
        tag_weights,
        tagged_pages,
        site_changes,
    };
    Ok((handle, wanted.tag_cloud))
}

async fn fetch_page_titles(
    ctx: &ServiceContext<'_>,
    references: BTreeSet<(String, String)>,
) -> Result<BTreeMap<(String, String), String>> {
    if references.is_empty() {
        return Ok(BTreeMap::new());
    }
    let query = build_title_query(references);
    let rows = Page::find()
        .from_raw_sql(query)
        .into_model::<PageTitle>()
        .all(ctx.transaction())
        .await
        .or_raise(|| {
            Error::new("failed to fetch referenced page titles", ErrorType::Render)
        })?;
    Ok(rows
        .into_iter()
        .map(|row| ((row.site, row.page), row.title))
        .collect())
}

/// Wikidot's tag cloud: tags of live pages' current revisions, hidden (`_`)
/// tags excluded, in the glibc en_US order Wikidot's database sorts by.
async fn fetch_tag_weights(
    ctx: &ServiceContext<'_>,
    site: &str,
) -> Result<Vec<(String, u64)>> {
    let query = Statement::from_sql_and_values(
        DatabaseBackend::Postgres,
        format!(
            "SELECT tag, COUNT(*) AS weight
             FROM {LIVE_PAGES}, unnest(r.tags) AS tag
             WHERE left(tag, 1) <> '_'
             GROUP BY tag
             ORDER BY tag COLLATE \"en_US.utf8\""
        ),
        [Value::from(site)],
    );
    let rows = Page::find()
        .from_raw_sql(query)
        .into_model::<TagWeight>()
        .all(ctx.transaction())
        .await
        .or_raise(|| Error::new("failed to fetch site tags", ErrorType::Render))?;
    Ok(rows
        .into_iter()
        .map(|row| (row.tag, row.weight.unsigned_abs()))
        .collect())
}

/// Wikidot's PagesByTag: pages whose current revision has the tag, by title.
async fn fetch_tagged_pages(
    ctx: &ServiceContext<'_>,
    site: &str,
    tag: &str,
) -> Result<Vec<(String, String)>> {
    let query = Statement::from_sql_and_values(
        DatabaseBackend::Postgres,
        format!(
            "SELECT p.slug, r.title
             FROM {LIVE_PAGES}
             WHERE $2 = ANY(r.tags)
             ORDER BY COALESCE(NULLIF(r.title, ''), p.slug) COLLATE \"en_US.utf8\""
        ),
        [Value::from(site), Value::from(tag)],
    );
    let rows = Page::find()
        .from_raw_sql(query)
        .into_model::<TaggedPage>()
        .all(ctx.transaction())
        .await
        .or_raise(|| Error::new("failed to fetch tagged pages", ErrorType::Render))?;
    Ok(rows.into_iter().map(|row| (row.slug, row.title)).collect())
}

/// One page of Wikidot's revision list, newest first, with the category names
/// its filter form lists.
async fn fetch_site_changes(
    ctx: &ServiceContext<'_>,
    site: &str,
    page_fullname: String,
    list_page: usize,
    filter: &SiteChangesFilter,
) -> Result<SiteChanges> {
    let make_error = || Error::new("failed to fetch site changes", ErrorType::Render);
    let per_page = filter.per_page;
    let offset = (list_page.max(1) - 1) * per_page;
    // Wikidot's revision-type and category filters.
    let types: Vec<String> = filter.types.chars().map(String::from).collect();
    let rows = Page::find()
        .from_raw_sql(Statement::from_sql_and_values(
            DatabaseBackend::Postgres,
            "SELECT c.page_slug, c.page_title, c.flags,
                    EXTRACT(EPOCH FROM c.changed_at)::bigint AS changed_at,
                    c.revision_number, c.user_slug, c.user_name, c.comments,
                    COUNT(*) OVER () AS total
             FROM wikidot_site_change c
             JOIN site s ON s.site_id = c.site_id AND s.slug = $1
             WHERE (cardinality($4::text[]) = 0
                    OR EXISTS (SELECT 1 FROM unnest($4::text[]) AS t(flag)
                               WHERE strpos(c.flags, t.flag) > 0))
               AND ($5::text IS NULL
                    OR (strpos(c.page_slug, ':') > 0 AND split_part(c.page_slug, ':', 1) = $5)
                    OR ($5 = '_default' AND strpos(c.page_slug, ':') = 0))
             ORDER BY c.changed_at DESC, c.revision_number DESC, c.page_slug
             LIMIT $2 OFFSET $3",
            [
                Value::from(site),
                Value::from(per_page as i64),
                Value::from(offset as i64),
                Value::from(types),
                Value::from(filter.category.clone()),
            ],
        ))
        .into_model::<SiteChangeRow>()
        .all(ctx.transaction())
        .await
        .or_raise(make_error)?;
    let categories = Page::find()
        .from_raw_sql(Statement::from_sql_and_values(
            DatabaseBackend::Postgres,
            "SELECT c.slug FROM page_category c
             JOIN site s ON s.site_id = c.site_id AND s.slug = $1
             ORDER BY c.slug COLLATE \"C\"",
            [Value::from(site)],
        ))
        .into_model::<CategorySlug>()
        .all(ctx.transaction())
        .await
        .or_raise(make_error)?;
    let total = rows
        .first()
        .map_or(0, |row| row.total.unsigned_abs() as usize);
    Ok(SiteChanges {
        page_fullname,
        list_page,
        page_count: total.div_ceil(per_page),
        per_page,
        category: filter.category.clone(),
        types: filter.types.clone(),
        categories: categories.into_iter().map(|row| row.slug).collect(),
        changes: rows
            .into_iter()
            .map(|row| SiteChange {
                slug: row.page_slug,
                title: row.page_title,
                flags: row.flags,
                changed_at: row.changed_at,
                revision: row.revision_number,
                user_slug: row.user_slug,
                user_name: row.user_name,
                comments: row.comments,
            })
            .collect(),
    })
}

/// Live pages of site `$1` joined to their current revision `r`.
const LIVE_PAGES: &str = "site s
    JOIN page p ON p.site_id = s.site_id AND p.deleted_at IS NULL
        AND s.slug = $1 AND s.deleted_at IS NULL
    JOIN LATERAL (
        SELECT title, tags FROM page_revision
        WHERE site_id = p.site_id AND page_id = p.page_id
        ORDER BY revision_number DESC LIMIT 1
    ) r ON true";

fn build_title_query(references: BTreeSet<(String, String)>) -> Statement {
    let (sites, pages): (Vec<_>, Vec<_>) = references.into_iter().unzip();
    Statement::from_sql_and_values(
        DatabaseBackend::Postgres,
        "SELECT s.slug AS site, p.slug AS page, r.title
         FROM unnest($1::text[], $2::text[]) AS wanted(site, page)
         JOIN site s ON s.slug = wanted.site AND s.deleted_at IS NULL
         JOIN page p ON p.site_id = s.site_id AND p.slug = wanted.page AND p.deleted_at IS NULL
         JOIN LATERAL (
             SELECT title FROM page_revision
             WHERE site_id = p.site_id AND page_id = p.page_id
             ORDER BY revision_number DESC LIMIT 1
         ) r ON true",
        [Value::from(sites), Value::from(pages)],
    )
}

fn collect_wanted(tree: &SyntaxTree<'_>, site: &str) -> Wanted {
    let mut references = Wanted::default();
    collect_elements(&tree.elements, site, &mut references);
    collect_elements(&tree.table_of_contents, site, &mut references);
    for footnote in &tree.footnotes {
        collect_elements(footnote, site, &mut references);
    }
    for index in 0..tree.bibliographies.next_index() {
        for (_, elements) in tree.bibliographies.get_bibliography(index).slice() {
            collect_elements(elements, site, &mut references);
        }
    }
    references
}

fn collect_elements(elements: &[Element<'_>], site: &str, references: &mut Wanted) {
    for element in elements {
        collect_element(element, site, references);
    }
}

fn collect_element(element: &Element<'_>, site: &str, references: &mut Wanted) {
    match element {
        Element::Link {
            link: LinkLocation::Page(page),
            label: LinkLabel::Page,
            ..
        } => {
            let (site, page, _) = page.fields_or(site);
            references.pages.insert((site.to_owned(), page.to_owned()));
        }
        Element::Module(Module::TagCloud { .. }) => references.tag_cloud = true,
        Element::Module(Module::PagesByTag) => references.pages_by_tag = true,
        Element::Module(Module::SiteChanges) => references.site_changes = true,
        Element::Container(container) => {
            collect_elements(container.elements(), site, references)
        }
        Element::Anchor { elements, .. }
        | Element::Collapsible { elements, .. }
        | Element::Include { elements, .. } => {
            collect_elements(elements, site, references)
        }
        Element::List { items, .. } => collect_list(items, site, references),
        Element::Table(table) => collect_table(table, site, references),
        Element::TabView(tabs) => collect_tabs(tabs, site, references),
        Element::DefinitionList(items) => collect_definitions(items, site, references),
        _ => {}
    }
}

fn collect_table(table: &Table<'_>, site: &str, references: &mut Wanted) {
    for cell in table.rows.iter().flat_map(|row| &row.cells) {
        collect_elements(&cell.elements, site, references);
    }
}

fn collect_tabs(tabs: &[Tab<'_>], site: &str, references: &mut Wanted) {
    for tab in tabs {
        collect_elements(&tab.elements, site, references);
    }
}

fn collect_definitions(
    items: &[DefinitionListItem<'_>],
    site: &str,
    references: &mut Wanted,
) {
    for item in items {
        collect_elements(&item.key_elements, site, references);
        collect_elements(&item.value_elements, site, references);
    }
}

fn collect_list(items: &[ListItem<'_>], site: &str, references: &mut Wanted) {
    for item in items {
        match item {
            ListItem::Elements { elements, .. } => {
                collect_elements(elements, site, references)
            }
            ListItem::SubList { element } => collect_element(element, site, references),
        }
    }
}
