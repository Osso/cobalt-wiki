//! Creating a page in a category whose `_template` defines a data form: the
//! page source is the record Wikidot saves from that form.

use crate::error::prelude::*;
use crate::services::page::CreatePage;
use crate::services::view::{extract_page_form, template_slug};
use crate::services::{ServiceContext, ViewService};
use crate::types::Maybe;
use wikidot_forms::{FormView, Mapping, new_record};

#[derive(Debug, Deserialize)]
pub(super) struct CreatePageRequest {
    #[serde(flatten)]
    pub create: CreatePage,
    #[serde(default)]
    pub form_updates: Maybe<Mapping>,
}

impl CreatePageRequest {
    pub(super) async fn load_create(
        mut self,
        ctx: &ServiceContext<'_>,
    ) -> Result<CreatePage> {
        let Maybe::Set(updates) = &self.form_updates else {
            return Ok(self.create);
        };
        if !self.create.wikitext.is_empty() {
            return Err(Error::new(
                "wikitext and form_updates cannot be combined",
                ErrorType::BadRequest,
            )
            .into());
        }
        self.create.wikitext =
            load_new_record(ctx, self.create.site_id, &self.create.slug, updates).await?;
        Ok(self.create)
    }
}

/// The source of a new page at `slug` saved from its category form.
pub(crate) async fn load_new_record(
    ctx: &ServiceContext<'_>,
    site_id: i64,
    slug: &str,
    updates: &Mapping,
) -> Result<String> {
    let form = load_create_form(ctx, site_id, slug).await?.ok_or_else(|| {
        Error::new(
            "form_updates requires a visible category form template",
            ErrorType::BadRequest,
        )
    })?;
    new_record(&form.schema, updates)
        .or_raise(|| Error::new("invalid form values", ErrorType::BadRequest))
}

/// The category form a new page at `slug` is created from, as the requesting
/// user may see it; values are empty so the editor shows field defaults.
pub(super) async fn load_create_form(
    ctx: &ServiceContext<'_>,
    site_id: i64,
    slug: &str,
) -> Result<Option<FormView>> {
    let Some(template) = template_slug(slug) else {
        return Ok(None);
    };
    let source = ViewService::load_visible_template_source(
        ctx,
        site_id,
        ctx.request().user_id,
        &template,
    )
    .await?;
    extract_page_form(source.as_deref(), "{}")
        .or_raise(|| Error::new("failed to extract category form", ErrorType::BadRequest))
}
