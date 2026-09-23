use super::prelude::*;
use crate::models::page::Entity as Page;
use ftml::tree::{
    DefinitionListItem, Element, LinkLabel, LinkLocation, ListItem, SyntaxTree, Tab,
    Table,
};
use sea_orm::{DatabaseBackend, FromQueryResult, Statement, Value};
use std::collections::{BTreeMap, BTreeSet};

#[derive(Debug, FromQueryResult)]
struct PageTitle {
    site: String,
    page: String,
    title: String,
}

pub(super) async fn fetch_page_titles(
    ctx: &ServiceContext<'_>,
    tree: &SyntaxTree<'_>,
    site: &str,
) -> Result<BTreeMap<(String, String), String>> {
    let references = collect_page_references(tree, site);
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

fn collect_page_references(
    tree: &SyntaxTree<'_>,
    site: &str,
) -> BTreeSet<(String, String)> {
    let mut references = BTreeSet::new();
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

fn collect_elements(
    elements: &[Element<'_>],
    site: &str,
    references: &mut BTreeSet<(String, String)>,
) {
    for element in elements {
        collect_element(element, site, references);
    }
}

fn collect_element(
    element: &Element<'_>,
    site: &str,
    references: &mut BTreeSet<(String, String)>,
) {
    match element {
        Element::Link {
            link: LinkLocation::Page(page),
            label: LinkLabel::Page,
            ..
        } => {
            let (site, page, _) = page.fields_or(site);
            references.insert((site.to_owned(), page.to_owned()));
        }
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

fn collect_table(
    table: &Table<'_>,
    site: &str,
    references: &mut BTreeSet<(String, String)>,
) {
    for cell in table.rows.iter().flat_map(|row| &row.cells) {
        collect_elements(&cell.elements, site, references);
    }
}

fn collect_tabs(
    tabs: &[Tab<'_>],
    site: &str,
    references: &mut BTreeSet<(String, String)>,
) {
    for tab in tabs {
        collect_elements(&tab.elements, site, references);
    }
}

fn collect_definitions(
    items: &[DefinitionListItem<'_>],
    site: &str,
    references: &mut BTreeSet<(String, String)>,
) {
    for item in items {
        collect_elements(&item.key_elements, site, references);
        collect_elements(&item.value_elements, site, references);
    }
}

fn collect_list(
    items: &[ListItem<'_>],
    site: &str,
    references: &mut BTreeSet<(String, String)>,
) {
    for item in items {
        match item {
            ListItem::Elements { elements, .. } => {
                collect_elements(elements, site, references)
            }
            ListItem::SubList { element } => collect_element(element, site, references),
        }
    }
}
