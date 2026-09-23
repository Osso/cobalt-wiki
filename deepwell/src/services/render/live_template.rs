//! Wikidot live form templates: a category's `_template` wraps each YAML page record.

use super::includes::fetch_shared_source_by_slug;
use super::list_pages::list_pages_blocks;
use super::page_tokens::{FormRecord, PageTokens, substitute_tokens};
use super::prelude::*;
use crate::services::SiteService;
use crate::services::view::form::template_slug;
use crate::types::Reference;
use wikidot_forms::{parse_schema, parse_values, split_template};

/// Replace a form page's YAML record with its category template, filled from that record.
///
/// Hidden pages (`_template`, `_public`) and pages without a readable form
/// template keep their own source unchanged.
pub(super) async fn apply_live_template(
    ctx: &ServiceContext<'_>,
    source: String,
    page_info: &PageInfo<'_>,
) -> Result<String> {
    if page_info.page.starts_with('_') {
        return Ok(source);
    }
    let fullname = match &page_info.category {
        Some(category) => format!("{category}:{}", page_info.page),
        None => page_info.page.to_string(),
    };
    let Some(slug) = template_slug(&fullname) else {
        return Ok(source);
    };
    let site =
        SiteService::get(ctx, Reference::Slug(page_info.site.as_ref().into())).await?;
    let Some(template) = fetch_shared_source_by_slug(ctx, site.site_id, &slug).await?
    else {
        return Ok(source);
    };
    let parts = split_template(&template).or_raise(|| {
        Error::new("live template has invalid form markers", ErrorType::Render)
    })?;
    let Some(definition) = parts.definition else {
        return Ok(source);
    };
    let schema = parse_schema(&definition).or_raise(|| {
        Error::new("live template has an invalid form", ErrorType::Render)
    })?;
    let values = parse_values(&source).or_raise(|| {
        Error::new("form page is not a valid field record", ErrorType::Render)
    })?;
    let form = FormRecord {
        schema: &schema,
        values,
    };
    let tokens = PageTokens {
        fullname: &fullname,
        title: &page_info.title,
        created_at: None,
        updated_at: None,
        form: Some(&form),
    };
    Ok(fill_template(live_section(&parts.body), &tokens))
}

/// The template text shown on pages is everything before the first `====` line.
fn live_section(body: &str) -> &str {
    let mut offset = 0;
    for line in body.split_inclusive('\n') {
        if line.trim_end() == "====" {
            return &body[..offset];
        }
        offset += line.len();
    }
    body
}

/// Substitute this page's tokens everywhere except ListPages item templates,
/// whose tokens describe each listed page instead.
fn fill_template(template: &str, tokens: &PageTokens) -> String {
    let mut output = String::with_capacity(template.len());
    let mut copied = 0;
    for block in list_pages_blocks(template) {
        output.push_str(&substitute_tokens(
            &template[copied..block.body_range.start],
            tokens,
        ));
        output.push_str(block.body);
        copied = block.body_range.end;
    }
    output.push_str(&substitute_tokens(&template[copied..], tokens));
    output
}

#[cfg(test)]
mod tests {
    use super::*;

    fn tokens() -> PageTokens<'static> {
        PageTokens {
            fullname: "character:atley",
            title: "Sir Dane Atley",
            created_at: None,
            updated_at: None,
            form: None,
        }
    }

    #[test]
    fn live_section_stops_at_the_separator_line() {
        assert_eq!(
            live_section("Top %%title%%\n\n====\n\nBelow\n"),
            "Top %%title%%\n\n"
        );
        assert_eq!(live_section("No separator ===="), "No separator ====");
    }

    #[test]
    fn list_pages_item_tokens_describe_listed_pages() {
        let template = "+ %%title%%\n[[module ListPages tags=\"+%%name%%\"]]\n* %%linked_title%%\n[[/module]]\n%%fullname%%";
        assert_eq!(
            fill_template(template, &tokens()),
            "+ Sir Dane Atley\n[[module ListPages tags=\"+atley\"]]\n* %%linked_title%%\n[[/module]]\ncharacter:atley",
        );
    }
}
