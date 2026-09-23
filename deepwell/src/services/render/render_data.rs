use super::prelude::*;
use crate::models::page::Entity as Page;
use ftml::render::Handle;
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
}

/// Fetch the data FTML renders from: titles of linked pages, plus site tags
/// for TagCloud and the pages carrying the URL's `tag` for PagesByTag. The
/// flag is set when the output depends on every page's tags (TagCloud).
pub(super) async fn fetch_render_data(
    ctx: &ServiceContext<'_>,
    tree: &SyntaxTree<'_>,
    site: &str,
    tag: Option<&str>,
) -> Result<(Handle, bool)> {
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
    let handle = Handle {
        page_titles,
        tag_weights,
        tagged_pages,
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
