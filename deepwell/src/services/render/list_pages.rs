//! Wikidot `[[module ListPages]]` and `[[module CountPages]]` expansion into ordinary
//! wikitext before FTML parsing.

use super::includes::can_view_shared;
use super::page_tokens::{FormRecord, PageTokens, substitute_tokens};

/// Substitute tokens everywhere except nested module item templates, whose
/// tokens describe the pages those modules list.
pub(super) fn substitute_outside_module_bodies(
    text: &str,
    tokens: &PageTokens,
) -> String {
    let mut output = String::with_capacity(text.len());
    let mut copied = 0;
    for block in list_pages_blocks(text) {
        output.push_str(&substitute_tokens(
            &text[copied..block.body_range.start],
            tokens,
        ));
        output.push_str(block.body);
        copied = block.body_range.end;
    }
    output.push_str(&substitute_tokens(&text[copied..], tokens));
    output
}
use super::prelude::*;
use crate::models::page::Entity as Page;
use crate::models::text::{self, Entity as Text};
use crate::services::SiteService;
use crate::services::view::form::template_slug;
use crate::types::Reference;
use sea_orm::{DatabaseBackend, FromQueryResult, Statement, Value};
use std::collections::HashMap;
use std::ops::Range;
use time::{Duration, OffsetDateTime};
use wikidot_forms::{parse_schema, parse_values, split_template};

/// Module argument errors are shown to readers in place of the list.
type ParseResult<T> = std::result::Result<T, String>;

const DEFAULT_PER_PAGE: usize = 20;
const MAX_PER_PAGE: usize = 250;
/// Nested listing levels expanded; deeper modules stay as text.
const MAX_NESTING: usize = 4;

#[derive(Debug, PartialEq, Clone, Copy)]
pub(super) enum ModuleKind {
    ListPages,
    CountPages,
}

/// One module occurrence: its whole source range, header text, and item template.
#[derive(Debug, PartialEq)]
pub(super) struct ListPagesBlock<'t> {
    pub kind: ModuleKind,
    pub range: Range<usize>,
    pub header: &'t str,
    pub body: &'t str,
    pub body_range: Range<usize>,
}

/// Find every terminated ListPages/CountPages module. Unterminated modules remain
/// ordinary text.
pub(super) fn list_pages_blocks(source: &str) -> Vec<ListPagesBlock<'_>> {
    let mut blocks = Vec::new();
    let lower = source.to_ascii_lowercase();
    let mut offset = 0;
    while let Some(block) = next_block(source, &lower, offset) {
        offset = block.range.end;
        blocks.push(block);
    }
    blocks
}

fn next_block<'t>(
    source: &'t str,
    lower: &str,
    from: usize,
) -> Option<ListPagesBlock<'t>> {
    let mut search = from;
    loop {
        let start = search + lower[search..].find("[[module")?;
        let name_start = start + "[[module".len();
        let Some((module, kind)) = module_at(lower, name_start) else {
            search = name_start;
            continue;
        };
        let name = lower[name_start..].trim_start();
        let header_start = source.len() - name.len() + module.len();
        let header_end = header_start + find_header_end(&source[header_start..])?;
        let body_start = header_end + 2;
        let body_end = find_matching_close(lower, body_start)?;
        return Some(ListPagesBlock {
            kind,
            range: start..body_end + "[[/module]]".len(),
            header: &source[header_start..header_end],
            body: &source[body_start..body_end],
            body_range: body_start..body_end,
        });
    }
}

/// The ListPages/CountPages module named after a `[[module` opening, if any.
fn module_at(lower: &str, name_start: usize) -> Option<(&'static str, ModuleKind)> {
    if !lower[name_start..].starts_with(char::is_whitespace) {
        return None;
    }
    let name = lower[name_start..].trim_start();
    [
        ("listpages", ModuleKind::ListPages),
        ("countpages", ModuleKind::CountPages),
    ]
    .into_iter()
    .find(|(module, _)| {
        name.starts_with(module)
            && name[module.len()..].starts_with(|c: char| c.is_whitespace() || c == ']')
    })
}

/// The `[[/module]]` closing a body starting at `body_start`, skipping the
/// bodies of nested ListPages/CountPages modules.
fn find_matching_close(lower: &str, body_start: usize) -> Option<usize> {
    let mut depth = 0;
    let mut offset = body_start;
    loop {
        let close = offset + lower[offset..].find("[[/module]]")?;
        let open = lower[offset..close]
            .match_indices("[[module")
            .map(|(index, _)| offset + index)
            .find(|&open| module_at(lower, open + "[[module".len()).is_some());
        match open {
            Some(open) => {
                depth += 1;
                offset = open + "[[module".len();
            }
            None if depth == 0 => return Some(close),
            None => {
                depth -= 1;
                offset = close + "[[/module]]".len();
            }
        }
    }
}

/// The first `]]` outside a double-quoted value; quoted values may contain `]]`.
fn find_header_end(header: &str) -> Option<usize> {
    let mut quoted = false;
    let bytes = header.as_bytes();
    for index in 0..bytes.len() {
        match bytes[index] {
            b'"' => quoted = !quoted,
            b']' if !quoted && bytes.get(index + 1) == Some(&b']') => return Some(index),
            _ => {}
        }
    }
    None
}

/// Parse `key="value"` / `key=value` pairs. Stray `\` continuation marks are ignored.
pub(super) fn parse_attributes(header: &str) -> ParseResult<HashMap<String, String>> {
    let mut attributes = HashMap::new();
    let mut rest = header;
    loop {
        rest = rest.trim_start_matches(|c: char| c.is_whitespace() || c == '\\');
        if rest.is_empty() {
            return Ok(attributes);
        }
        let key_length = rest
            .find(|c: char| !(c.is_ascii_alphanumeric() || c == '_'))
            .unwrap_or(rest.len());
        let key = &rest[..key_length];
        let Some(after_equals) = rest[key_length..].strip_prefix('=') else {
            return Err("malformed module arguments".into());
        };
        if key.is_empty() {
            return Err("malformed module arguments".into());
        }
        let (value, remaining) = match after_equals.strip_prefix('"') {
            Some(quoted) => {
                let end = quoted.find('"').ok_or("unterminated quoted argument")?;
                (&quoted[..end], &quoted[end + 1..])
            }
            None => {
                let end = after_equals
                    .find(char::is_whitespace)
                    .unwrap_or(after_equals.len());
                after_equals.split_at(end)
            }
        };
        attributes.insert(key.to_ascii_lowercase(), value.to_owned());
        rest = remaining;
    }
}

#[derive(Debug, PartialEq)]
enum PageType {
    Normal,
    Hidden,
    All,
}

#[derive(Debug, PartialEq, Clone, Copy)]
enum OrderField {
    Name,
    Fullname,
    Title,
    CreatedAt,
    UpdatedAt,
}

/// What to select and how to lay it out, derived from module arguments.
#[derive(Debug, PartialEq)]
pub(super) struct Selection {
    all_categories: bool,
    categories: Vec<String>,
    excluded_categories: Vec<String>,
    any_tags: Vec<String>,
    all_tags: Vec<String>,
    excluded_tags: Vec<String>,
    untagged: bool,
    page_type: PageType,
    order: OrderField,
    descending: bool,
    per_page: usize,
    /// Cap on items across all pages.
    limit: Option<usize>,
    created_within_days: Option<i64>,
    separate: bool,
    prepend_line: Option<String>,
    append_line: Option<String>,
}

pub(super) fn parse_selection(
    attributes: &HashMap<String, String>,
    current_category: &str,
) -> ParseResult<Selection> {
    for key in attributes.keys() {
        if !matches!(
            key.as_str(),
            "category"
                | "tags"
                | "tag"
                | "pagetype"
                | "order"
                | "limit"
                | "perpage"
                | "created_at"
                | "separate"
                | "prependline"
                | "appendline"
        ) {
            return Err(format!("unsupported argument {key}"));
        }
    }
    if attributes.contains_key("tags") && attributes.contains_key("tag") {
        return Err("both tags and tag given".into());
    }
    let mut selection = Selection {
        all_categories: false,
        categories: Vec::new(),
        excluded_categories: Vec::new(),
        any_tags: Vec::new(),
        all_tags: Vec::new(),
        excluded_tags: Vec::new(),
        untagged: false,
        page_type: parse_page_type(attributes.get("pagetype"))?,
        order: OrderField::CreatedAt,
        descending: true,
        per_page: parse_number(attributes, "perpage")?
            .unwrap_or(DEFAULT_PER_PAGE)
            .min(MAX_PER_PAGE),
        limit: parse_number(attributes, "limit")?,
        created_within_days: attributes
            .get("created_at")
            .map(|value| parse_last_days(value))
            .transpose()?,
        separate: parse_separate(attributes.get("separate"))?,
        prepend_line: attributes.get("prependline").cloned(),
        append_line: attributes.get("appendline").cloned(),
    };
    parse_categories(attributes.get("category"), current_category, &mut selection);
    if let Some(tags) = attributes.get("tags").or_else(|| attributes.get("tag")) {
        parse_tags(tags, &mut selection);
    }
    if let Some(order) = attributes.get("order") {
        (selection.order, selection.descending) = parse_order(order)?;
    }
    Ok(selection)
}

fn parse_categories(value: Option<&String>, current: &str, selection: &mut Selection) {
    let value = value
        .map(|value| value.trim())
        .filter(|value| !value.is_empty());
    let Some(value) = value else {
        selection.categories.push(current.to_owned());
        return;
    };
    for term in value.split_whitespace() {
        match term {
            "*" => selection.all_categories = true,
            "." => selection.categories.push(current.to_owned()),
            _ => match term.strip_prefix('-') {
                Some(excluded) => selection.excluded_categories.push(excluded.to_owned()),
                None => selection
                    .categories
                    .push(term.trim_start_matches('+').to_owned()),
            },
        }
    }
    if selection.categories.is_empty() {
        selection.all_categories = true;
    }
}

fn parse_tags(value: &str, selection: &mut Selection) {
    for term in value.split_whitespace() {
        if term == "-" {
            selection.untagged = true;
        } else if let Some(tag) = term.strip_prefix('+') {
            selection.all_tags.push(tag.to_owned());
        } else if let Some(tag) = term.strip_prefix('-') {
            selection.excluded_tags.push(tag.to_owned());
        } else {
            selection.any_tags.push(term.to_owned());
        }
    }
}

fn parse_page_type(value: Option<&String>) -> ParseResult<PageType> {
    match value.map(String::as_str) {
        None | Some("normal") => Ok(PageType::Normal),
        Some("hidden") => Ok(PageType::Hidden),
        Some("*") => Ok(PageType::All),
        Some(_) => Err("unsupported pagetype".into()),
    }
}

fn parse_order(value: &str) -> ParseResult<(OrderField, bool)> {
    let mut terms = value.split_whitespace();
    let field = match terms.next() {
        Some("name") => OrderField::Name,
        Some("fullname") => OrderField::Fullname,
        Some("title") => OrderField::Title,
        Some("created_at") => OrderField::CreatedAt,
        Some("updated_at") => OrderField::UpdatedAt,
        _ => return Err("unsupported order".into()),
    };
    let descending = match terms.next() {
        None | Some("asc") => false,
        Some("desc") => true,
        Some(_) => return Err("unsupported order".into()),
    };
    if terms.next().is_some() {
        return Err("unsupported order".into());
    }
    Ok((field, descending))
}

fn parse_number(
    attributes: &HashMap<String, String>,
    key: &str,
) -> ParseResult<Option<usize>> {
    attributes
        .get(key)
        .map(|value| {
            value
                .trim()
                .parse::<usize>()
                .map_err(|_| format!("invalid {key}"))
        })
        .transpose()
}

fn parse_last_days(value: &str) -> ParseResult<i64> {
    let terms: Vec<_> = value.split_whitespace().collect();
    match terms.as_slice() {
        ["last", days, "day" | "days"] => days
            .parse()
            .map_err(|_| "unsupported created_at".to_owned()),
        _ => Err("unsupported created_at".into()),
    }
}

fn parse_separate(value: Option<&String>) -> ParseResult<bool> {
    match value.map(String::as_str) {
        None | Some("yes" | "true") => Ok(true),
        Some("no" | "false") => Ok(false),
        Some(_) => Err("unsupported separate".into()),
    }
}

#[derive(Debug, Clone, FromQueryResult)]
struct ListedPage {
    page_id: i64,
    page_category_id: i64,
    slug: String,
    category: String,
    created_at: OffsetDateTime,
    updated_at: Option<OffsetDateTime>,
    title: String,
    tags: Vec<String>,
    wikitext_hash: Vec<u8>,
}

impl ListedPage {
    fn name(&self) -> &str {
        crate::utils::split_category(&self.slug).1
    }
}

/// Replace every ListPages module in rendered wikitext with its generated items.
pub(super) async fn expand_list_pages(
    ctx: &ServiceContext<'_>,
    mut source: String,
    page_info: &PageInfo<'_>,
    list_page: usize,
) -> Result<String> {
    if list_pages_blocks(&source).is_empty() {
        return Ok(source);
    }
    let site =
        SiteService::get(ctx, Reference::Slug(page_info.site.as_ref().into())).await?;
    let mut listing = SiteListing::load(ctx, site.site_id).await?;
    // Items of an outer module may contain inner modules (nested listings).
    for _ in 0..MAX_NESTING {
        match expand_list_pages_once(ctx, &source, page_info, &mut listing, list_page)
            .await?
        {
            Some(expanded) => source = expanded,
            None => break,
        }
    }
    Ok(source)
}

/// Expand the outermost modules; `None` when there are none.
async fn expand_list_pages_once(
    ctx: &ServiceContext<'_>,
    source: &str,
    page_info: &PageInfo<'_>,
    listing: &mut SiteListing,
    list_page: usize,
) -> Result<Option<String>> {
    let blocks = list_pages_blocks(source);
    if blocks.is_empty() {
        return Ok(None);
    }
    let site_id = listing.site_id;
    let current_category = page_info.category.as_deref().unwrap_or("_default");
    let fullname = match current_category {
        "_default" => page_info.page.to_string(),
        category => format!("{category}:{}", page_info.page),
    };
    let mut forms = FormCache::default();
    let mut output = String::with_capacity(source.len());
    let mut copied = 0;
    for block in &blocks {
        output.push_str(&source[copied..block.range.start]);
        let selection = parse_attributes(block.header)
            .and_then(|attributes| parse_selection(&attributes, current_category));
        match (block.kind, selection) {
            (ModuleKind::ListPages, Ok(selection)) => {
                let pages = listing.select(ctx, &selection).await?;
                let page_count = pages.len().div_ceil(selection.per_page);
                let shown: Vec<_> = pages
                    .into_iter()
                    .skip((list_page - 1) * selection.per_page)
                    .take(selection.per_page)
                    .collect();
                let items =
                    render_items(ctx, block.body, &shown, &mut forms, site_id).await?;
                let pager = pager(&fullname, list_page, page_count);
                output.push_str(&layout_items(&selection, &items, &pager));
            }
            (ModuleKind::CountPages, Ok(selection)) => {
                let total = listing.select(ctx, &selection).await?.len();
                output.push_str(&layout_total(block.body, total));
            }
            (kind, Err(message)) => output.push_str(&error_block(kind, &message)),
        }
        copied = block.range.end;
    }
    output.push_str(&source[copied..]);
    Ok(Some(output))
}

fn error_block(kind: ModuleKind, message: &str) -> String {
    format!("[[div class=\"error-block\"]]\n{kind:?} module error: {message}.\n[[/div]]")
}

/// Every page of the site, loaded once per render: nested listings run one
/// selection per outer item, which as separate queries exceeded the render budget.
struct SiteListing {
    site_id: i64,
    pages: Vec<ListedPage>,
    category_visible: HashMap<i64, bool>,
}

impl SiteListing {
    async fn load(ctx: &ServiceContext<'_>, site_id: i64) -> Result<Self> {
        let statement = Statement::from_sql_and_values(
            DatabaseBackend::Postgres,
            "SELECT p.page_id, p.page_category_id, p.slug, c.slug AS category,
                    p.created_at, p.updated_at, r.title, r.tags, r.wikitext_hash
             FROM page p
             JOIN page_category c ON c.category_id = p.page_category_id
             JOIN page_revision r ON r.revision_id = p.latest_revision_id
             WHERE p.site_id = $1 AND p.deleted_at IS NULL",
            [Value::from(site_id)],
        );
        let pages = Page::find()
            .from_raw_sql(statement)
            .into_model::<ListedPage>()
            .all(ctx.transaction())
            .await
            .or_raise(|| {
                Error::new("failed to load ListPages pages", ErrorType::Render)
            })?;
        Ok(SiteListing {
            site_id,
            pages,
            category_visible: HashMap::new(),
        })
    }

    async fn select(
        &mut self,
        ctx: &ServiceContext<'_>,
        selection: &Selection,
    ) -> Result<Vec<ListedPage>> {
        let created_after = selection
            .created_within_days
            .map(|days| OffsetDateTime::now_utc() - Duration::days(days));
        let mut pages: Vec<&ListedPage> = self
            .pages
            .iter()
            .filter(|page| selection.matches(page, created_after))
            .collect();
        sort_pages(&mut pages, selection.order, selection.descending);
        // Compiled HTML is shared, so only anonymously readable pages may be listed.
        // Anonymous roles never depend on the page (page-author roles require a member),
        // so one check per category decides every page in it.
        let mut visible = Vec::new();
        for page in pages {
            if Some(visible.len()) == selection.limit {
                break;
            }
            let can_view = match self.category_visible.get(&page.page_category_id) {
                Some(&can_view) => can_view,
                None => {
                    let can_view = can_view_shared(
                        ctx,
                        self.site_id,
                        page.page_id,
                        page.page_category_id,
                    )
                    .await?;
                    self.category_visible
                        .insert(page.page_category_id, can_view);
                    can_view
                }
            };
            if can_view {
                visible.push(page.clone());
            }
        }
        Ok(visible)
    }
}

impl Selection {
    fn matches(&self, page: &ListedPage, created_after: Option<OffsetDateTime>) -> bool {
        let has = |tag: &String| page.tags.contains(tag);
        let hidden = page.name().starts_with('_');
        let page_type = match self.page_type {
            PageType::Normal => !hidden,
            PageType::Hidden => hidden,
            PageType::All => true,
        };
        page_type
            && (self.all_categories || self.categories.contains(&page.category))
            && !self.excluded_categories.contains(&page.category)
            && (self.any_tags.is_empty() || self.any_tags.iter().any(has))
            && self.all_tags.iter().all(has)
            && !self.excluded_tags.iter().any(has)
            && (!self.untagged || page.tags.is_empty())
            && created_after.is_none_or(|after| page.created_at >= after)
    }
}

fn sort_pages(pages: &mut [&ListedPage], order: OrderField, descending: bool) {
    pages.sort_by(|a, b| {
        let ordering = match order {
            OrderField::Name => a.name().to_lowercase().cmp(&b.name().to_lowercase()),
            OrderField::Fullname => a.slug.cmp(&b.slug),
            OrderField::Title => a.title.to_lowercase().cmp(&b.title.to_lowercase()),
            OrderField::CreatedAt => a.created_at.cmp(&b.created_at),
            OrderField::UpdatedAt => a.updated_at.cmp(&b.updated_at),
        };
        let ordering = if descending {
            ordering.reverse()
        } else {
            ordering
        };
        ordering.then_with(|| a.slug.cmp(&b.slug))
    });
}

/// Per-category form schemas, loaded only when an item template uses form tokens.
#[derive(Default)]
struct FormCache(HashMap<String, Option<wikidot_forms::FormSchema>>);

async fn render_items(
    ctx: &ServiceContext<'_>,
    body: &str,
    pages: &[ListedPage],
    forms: &mut FormCache,
    site_id: i64,
) -> Result<Vec<String>> {
    let body = body.trim_matches(|c| c == '\n' || c == '\r');
    let uses_forms = body.contains("%%form_data{") || body.contains("%%form_raw{");
    let records = match uses_forms {
        true => load_form_records(ctx, site_id, pages, forms).await?,
        false => pages.iter().map(|_| None).collect(),
    };
    let mut items = Vec::with_capacity(pages.len());
    for (page, form) in pages.iter().zip(&records) {
        let tokens = PageTokens {
            fullname: &page.slug,
            title: &page.title,
            created_at: Some(page.created_at.unix_timestamp()),
            updated_at: page.updated_at.map(OffsetDateTime::unix_timestamp),
            form: form.as_ref(),
        };
        items.push(substitute_outside_module_bodies(body, &tokens));
    }
    Ok(items)
}

/// Listed pages' form values, fetched in one query; pages whose source is not a
/// valid record, or whose category has no form, get none.
async fn load_form_records<'f>(
    ctx: &ServiceContext<'_>,
    site_id: i64,
    pages: &[ListedPage],
    forms: &'f mut FormCache,
) -> Result<Vec<Option<FormRecord<'f>>>> {
    for page in pages {
        if !forms.0.contains_key(&page.category) {
            let schema = load_category_schema(ctx, site_id, &page.slug).await?;
            forms.0.insert(page.category.clone(), schema);
        }
    }
    let forms = &*forms;
    let schema_of =
        |page: &ListedPage| forms.0.get(&page.category).and_then(Option::as_ref);
    let hashes: Vec<_> = pages
        .iter()
        .filter(|page| schema_of(page).is_some())
        .map(|page| page.wikitext_hash.clone())
        .collect();
    let sources: HashMap<_, _> = Text::find()
        .filter(text::Column::Hash.is_in(hashes))
        .all(ctx.transaction())
        .await
        .or_raise(|| {
            Error::new("failed to fetch listed page sources", ErrorType::Render)
        })?
        .into_iter()
        .map(|text| (text.hash, text.contents))
        .collect();
    Ok(pages
        .iter()
        .map(|page| {
            let schema = schema_of(page)?;
            let values = parse_values(sources.get(&page.wikitext_hash)?).ok()?;
            Some(FormRecord { schema, values })
        })
        .collect())
}

async fn load_category_schema(
    ctx: &ServiceContext<'_>,
    site_id: i64,
    page_slug: &str,
) -> Result<Option<wikidot_forms::FormSchema>> {
    let Some(slug) = template_slug(page_slug) else {
        return Ok(None);
    };
    let Some(template) =
        super::includes::fetch_shared_source_by_slug(ctx, site_id, &slug).await?
    else {
        return Ok(None);
    };
    let definition = split_template(&template)
        .or_raise(|| {
            Error::new(
                "category template has invalid form markers",
                ErrorType::Render,
            )
        })?
        .definition;
    definition
        .map(|definition| {
            parse_schema(&definition).or_raise(|| {
                Error::new("category template has an invalid form", ErrorType::Render)
            })
        })
        .transpose()
}

/// Wikidot joins unseparated items into one wikitext block; separated items are distinct blocks.
fn layout_items(selection: &Selection, items: &[String], pager: &str) -> String {
    if items.is_empty() {
        return String::new();
    }
    let mut lines = Vec::with_capacity(items.len() + 2);
    if selection.separate {
        lines.extend(
            items.iter().map(|item| {
                format!("[[div class=\"list-pages-item\"]]\n{item}\n[[/div]]")
            }),
        );
    } else {
        // Wikidot shows prepend/append lines only around joined items.
        lines.extend(selection.prepend_line.clone());
        lines.extend(items.iter().cloned());
        lines.extend(selection.append_line.clone());
    }
    let content = lines.join("\n");
    // A trailing line continuation must not join the closing div.
    let content = content.trim_end().trim_end_matches('\\').trim_end();
    format!("\n[[div class=\"list-pages-box\"]]\n{content}\n{pager}[[/div]]\n")
}

/// Wikidot's page navigation: pages 1–2, the current page ±2 and the last two,
/// with `...` over gaps; every module on the page follows the page's `/p/N`.
fn pager(fullname: &str, current: usize, page_count: usize) -> String {
    if page_count <= 1 {
        return String::new();
    }
    let link = |page: usize, label: &str| {
        format!("[[span class=\"target\"]][/{fullname}/p/{page} {label}][[/span]]")
    };
    let mut parts = vec![format!(
        "[[span class=\"pager-no\"]]page {current} of {page_count}[[/span]]"
    )];
    if current > 1 {
        parts.push(link(current - 1, "« previous"));
    }
    let shown =
        |page: usize| page <= 2 || page + 1 >= page_count || page.abs_diff(current) <= 2;
    let mut previous_shown = 0;
    for page in (1..=page_count).filter(|&page| shown(page)) {
        if page > previous_shown + 1 {
            parts.push("[[span class=\"dots\"]]...[[/span]]".into());
        }
        parts.push(match page == current {
            true => format!("[[span class=\"current\"]]{page}[[/span]]"),
            false => link(page, &page.to_string()),
        });
        previous_shown = page;
    }
    if current < page_count {
        parts.push(link(current + 1, "next »"));
    }
    format!("[[div class=\"pager\"]]\n{}\n[[/div]]\n", parts.join(""))
}

/// Wikidot fills `%%total%%` in the module body and boxes it like a listing.
fn layout_total(body: &str, total: usize) -> String {
    let body = body.trim_matches(|c| c == '\n' || c == '\r');
    let content = body.replace("%%total%%", &total.to_string());
    format!("\n[[div class=\"list-pages-box\"]]\n{content}\n[[/div]]\n")
}

#[cfg(test)]
mod tests {
    use super::*;

    fn selection(header: &str) -> Selection {
        parse_selection(&parse_attributes(header).unwrap(), "character").unwrap()
    }

    #[test]
    fn finds_multiline_headers_with_brackets_in_quoted_values() {
        let source = "Intro\n[[module ListPages category=\"player\" order=\"name\" perPage=\"200\"\nprependLine=\"+ %%title_linked%% [[# %%name%%]]\"]]\n* %%title%%\n[[/module]]\nOutro [[module Join]]";
        let blocks = list_pages_blocks(source);
        assert_eq!(blocks.len(), 1);
        let attributes = parse_attributes(blocks[0].header).unwrap();
        assert_eq!(
            attributes["prependline"],
            "+ %%title_linked%% [[# %%name%%]]"
        );
        assert_eq!(attributes["perpage"], "200");
        assert_eq!(blocks[0].body, "\n* %%title%%\n");
        assert_eq!(&source[blocks[0].range.end..], "\nOutro [[module Join]]");
    }

    #[test]
    fn ignores_other_modules_and_unterminated_list_pages() {
        assert!(list_pages_blocks("[[module ListPagesX]]x[[/module]]").is_empty());
        assert!(list_pages_blocks("[[module Join]] [[/module]]").is_empty());
        assert_eq!(
            list_pages_blocks("[[module CountPages]]%%total%%[[/module]]")[0].kind,
            ModuleKind::CountPages
        );
        assert!(
            list_pages_blocks("[[module ListPages category=\"a\"]] never closed")
                .is_empty()
        );
        assert_eq!(
            list_pages_blocks("[[MODULE listpages]]x[[/MODULE]]").len(),
            1
        );
    }

    #[test]
    fn stray_continuation_backslashes_are_not_arguments() {
        let attributes =
            parse_attributes(" separate=\"no\" category=\"player\" perPage=\"200\" \\")
                .unwrap();
        assert_eq!(attributes.len(), 3);
        assert_eq!(attributes["separate"], "no");
    }

    #[test]
    fn selects_categories_tags_and_limits_like_wikidot() {
        let nav = selection(
            " category=\"* -admin -home\" tag=\"guild-canon -_nav-omit\" order=\"title\" separate=\"no\"",
        );
        assert!(nav.all_categories);
        assert_eq!(nav.excluded_categories, ["admin", "home"]);
        assert_eq!(nav.any_tags, ["guild-canon"]);
        assert_eq!(nav.excluded_tags, ["_nav-omit"]);
        assert_eq!(
            (
                nav.order,
                nav.descending,
                nav.per_page,
                nav.limit,
                nav.separate
            ),
            (OrderField::Title, false, 20, None, false)
        );

        let home = selection(
            " category=\"character\" tags=\"_completed\" order=\"created_at asc\" limit=\"9\"",
        );
        assert_eq!(
            (home.categories.as_slice(), home.limit),
            (["character".to_owned()].as_slice(), Some(9))
        );
        assert_eq!(home.page_type, PageType::Normal);

        let required = selection(
            " category=\"writing +arc\" tags=\"+welcomed -reviewed\" perPage=\"500\" pagetype=\"*\"",
        );
        assert_eq!(required.categories, ["writing", "arc"]);
        assert_eq!(required.all_tags, ["welcomed"]);
        assert_eq!(
            (required.per_page, required.page_type),
            (250, PageType::All)
        );

        let current = selection(" tags=\"\" created_at=\"last 10 days\"");
        assert_eq!(current.categories, ["character"]);
        assert_eq!(
            (current.order, current.descending),
            (OrderField::CreatedAt, true)
        );
        assert_eq!(current.created_within_days, Some(10));
    }

    #[test]
    fn unsupported_arguments_are_explicit_errors() {
        let parse = |header| {
            parse_selection(&parse_attributes(header).unwrap(), "character").unwrap_err()
        };
        assert_eq!(parse(" rssTitle=\"x\""), "unsupported argument rsstitle");
        assert_eq!(parse(" order=\"rating desc\""), "unsupported order");
        assert_eq!(
            parse(" created_at=\"older than 3 days\""),
            "unsupported created_at"
        );
        assert_eq!(parse(" tags=\"a\" tag=\"b\""), "both tags and tag given");
        assert!(parse_attributes("category=\"open").is_err());
    }

    #[test]
    fn separated_and_joined_layouts() {
        let items = ["|| a ||".to_owned(), "|| b || \\".to_owned()];
        let joined = selection(" separate=\"no\" prependLine=\"||~ Head ||\"");
        assert_eq!(
            layout_items(&joined, &items, ""),
            "\n[[div class=\"list-pages-box\"]]\n||~ Head ||\n|| a ||\n|| b ||\n[[/div]]\n"
        );
        // Wikidot shows prepend/append lines only for joined items (Cobalt writings).
        let separate = selection(" prependLine=\"~ Page\" appendLine=\"End\"");
        assert_eq!(
            layout_items(&separate, &items[..1], ""),
            "\n[[div class=\"list-pages-box\"]]\n[[div class=\"list-pages-item\"]]\n|| a ||\n[[/div]]\n[[/div]]\n"
        );
        assert_eq!(layout_items(&joined, &[], ""), "");
    }

    /// Page lists of cobalt-company.wikidot.com/writings/p/N (46 pages).
    #[test]
    fn pager_matches_wikidot_page_navigation() {
        let labels = |current| {
            let wikitext = pager("writings", current, 46);
            let mut labels = Vec::new();
            for part in wikitext.split("[[span class=\"").skip(1) {
                let (class, rest) = part.split_once("\"]]").unwrap();
                let text = rest.split("[[/span]]").next().unwrap();
                labels.push(match class {
                    "target" => {
                        let (href, label) =
                            text[1..text.len() - 1].split_once(' ').unwrap();
                        format!("{label}={href}")
                    }
                    "current" => format!("({text})"),
                    _ => text.to_owned(),
                });
            }
            labels.join(" ")
        };
        assert_eq!(
            labels(1),
            "page 1 of 46 (1) 2=/writings/p/2 3=/writings/p/3 ... 45=/writings/p/45 46=/writings/p/46 next »=/writings/p/2"
        );
        assert_eq!(
            labels(10),
            "page 10 of 46 « previous=/writings/p/9 1=/writings/p/1 2=/writings/p/2 ... 8=/writings/p/8 9=/writings/p/9 (10) 11=/writings/p/11 12=/writings/p/12 ... 45=/writings/p/45 46=/writings/p/46 next »=/writings/p/11"
        );
        assert_eq!(
            labels(46),
            "page 46 of 46 « previous=/writings/p/45 1=/writings/p/1 2=/writings/p/2 ... 44=/writings/p/44 45=/writings/p/45 (46)"
        );
        assert_eq!(pager("writings", 1, 1), "");
    }

    /// Every archived module header must parse: `COBALT_ARCHIVE_SOURCE=<dir of *.txt>`.
    #[test]
    #[ignore = "requires the protected Cobalt source archive"]
    fn every_archived_header_is_supported() {
        let directory =
            std::env::var("COBALT_ARCHIVE_SOURCE").expect("archive directory");
        let mut modules = 0;
        let mut failures = Vec::new();
        for entry in std::fs::read_dir(directory).unwrap() {
            let path = entry.unwrap().path();
            let source = std::fs::read_to_string(&path).unwrap();
            for block in list_pages_blocks(&source) {
                modules += 1;
                let parsed = parse_attributes(block.header)
                    .and_then(|attributes| parse_selection(&attributes, "_default"));
                if let Err(message) = parsed {
                    failures.push(format!("{}: {message}", path.display()));
                }
            }
        }
        println!("parsed {modules} ListPages/CountPages modules");
        assert_eq!(modules, 81);
        assert!(failures.is_empty(), "{failures:#?}");
    }
}
