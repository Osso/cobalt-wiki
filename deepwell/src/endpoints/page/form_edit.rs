use crate::error::prelude::*;
use crate::services::page::{EditPage, check_last_revision};
use crate::services::view::{extract_page_form, template_slug};
use crate::services::{
    PageRevisionService, PageService, ServiceContext, TextService, ViewService,
};
use crate::types::{Maybe, Reference};
use wikidot_forms::{FormError, Mapping, apply_field_updates};

#[derive(Debug, Deserialize)]
pub(super) struct EditPageRequest<'a> {
    #[serde(flatten)]
    pub edit: EditPage<'a>,
    #[serde(default)]
    pub form_updates: Maybe<Mapping>,
    #[serde(default)]
    pub do_not_notify_watchers: bool,
}

impl<'a> EditPageRequest<'a> {
    fn validate_modes(&self) -> Result<()> {
        if self.form_updates.is_set() && self.edit.body.wikitext.is_set() {
            return Err(Error::new(
                "wikitext and form_updates cannot be combined",
                ErrorType::BadRequest,
            )
            .into());
        }
        Ok(())
    }

    /// Called only after page_edit's existing Action::Edit permission check.
    pub(super) async fn load_edit(
        mut self,
        ctx: &ServiceContext<'_>,
    ) -> Result<EditPage<'a>> {
        self.validate_modes()?;
        if let Maybe::Set(updates) = &self.form_updates {
            self.edit.body.wikitext = Maybe::Set(
                load_updated_source(
                    ctx,
                    self.edit.site_id,
                    self.edit.page.clone(),
                    self.edit.last_revision_id,
                    updates,
                )
                .await?,
            );
        }
        Ok(self.edit)
    }
}

pub(crate) async fn load_updated_source(
    ctx: &ServiceContext<'_>,
    site_id: i64,
    reference: Reference<'_>,
    last_revision_id: i64,
    updates: &Mapping,
) -> Result<String> {
    let page = PageService::get(ctx, site_id, reference).await?;
    check_last_revision(None, page.latest_revision_id, last_revision_id)?;
    let slug = template_slug(&page.slug).ok_or_else(|| {
        Error::new(
            "template pages cannot be edited using form_updates",
            ErrorType::BadRequest,
        )
    })?;
    let revision = PageRevisionService::get_latest(ctx, site_id, page.page_id).await?;
    // Reject a revision that advanced after the page-row read, before loading text.
    check_last_revision(None, Some(revision.revision_id), last_revision_id)?;
    let source = TextService::get(ctx, &revision.wikitext_hash).await?;
    let template = ViewService::load_visible_template_source(
        ctx,
        site_id,
        ctx.request().user_id,
        &slug,
    )
    .await?;
    apply_updates(template.as_deref(), &source, updates)
        .or_raise(|| Error::new("failed to apply form updates", ErrorType::BadRequest))
}

fn apply_updates(
    template: Option<&str>,
    source: &str,
    updates: &Mapping,
) -> std::result::Result<String, FormError> {
    let form = extract_page_form(template, source)?.ok_or_else(|| {
        FormError::Shape("form_updates requires a visible category form template".into())
    })?;
    apply_field_updates(&form.schema, &form.values, updates)
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;
    use wikidot_forms::parse_values;

    const TEMPLATE: &str = "[[form]]\nfields:\n  name:\n    type: text\n  active:\n    type: text\n  count:\n    type: text\n  notes:\n    type: wiki\n  kind:\n    type: select\n    values:\n      a: Alpha\n      b: Beta\n  fixed:\n    type: static\n[[/form]]";
    const SOURCE: &str = "name: Before\nactive: false\ncount: 3\nnotes: old\nkind: a\nfixed: Original\nunknown: true\n";

    fn request(fields: serde_json::Value) -> EditPageRequest<'static> {
        let mut value = json!({
            "site_id": 11,
            "page": "character:example",
            "last_revision_id": 42,
            "revision_comments": "Update fields",
            "user_id": 7,
            "ip_address": "127.0.0.10"
        });
        value
            .as_object_mut()
            .unwrap()
            .extend(fields.as_object().unwrap().clone());
        serde_json::from_value(value).unwrap()
    }

    #[test]
    fn raw_and_metadata_only_edits_keep_the_original_request() {
        for fields in [
            json!({"wikitext": "raw **wiki**", "title": "Title"}),
            json!({"tags": ["one"]}),
        ] {
            let request = request(fields.clone());
            request.validate_modes().unwrap();
            assert!(request.form_updates.is_unset());
            assert_eq!(request.edit.last_revision_id, 42);
            assert_eq!(request.edit.user_id, 7);
            assert_eq!(
                request.edit.body.wikitext.to_option().map(String::as_str),
                fields.get("wikitext").and_then(serde_json::Value::as_str)
            );
        }
    }

    #[test]
    fn rejects_combined_modes_even_when_both_are_empty() {
        for raw in ["", "name: Before"] {
            let request = request(json!({"wikitext": raw, "form_updates": {}}));
            let error = request.validate_modes().unwrap_err();
            let error = error.frame().error().downcast_ref::<Error>().unwrap();
            assert!(matches!(error.error_type, ErrorType::BadRequest));
        }
    }

    #[test]
    fn applies_wire_scalars_to_the_whole_record_without_coercion() {
        let request = request(json!({"form_updates": {
            "name": "After", "active": true, "count": 8,
            "notes": "[[include fragment]]\nsecond line", "kind": "b"
        }}));
        request.validate_modes().unwrap();
        let source = apply_updates(
            Some(TEMPLATE),
            SOURCE,
            request.form_updates.to_option().unwrap(),
        )
        .unwrap();
        let values = parse_values(&source).unwrap();
        assert_eq!(
            serde_json::to_value(values).unwrap(),
            json!({
                "name": "After", "active": true, "count": 8,
                "notes": "[[include fragment]]\nsecond line", "kind": "b",
                "fixed": "Original", "unknown": true
            })
        );
        assert_eq!(request.edit.last_revision_id, 42);
        assert_eq!(request.edit.user_id, 7);
    }

    #[test]
    fn requires_a_valid_visible_form_and_scalar_source() {
        let updates = parse_values("name: After").unwrap();
        for template in [None, Some("ordinary wiki"), Some("[[form]]")] {
            assert!(apply_updates(template, SOURCE, &updates).is_err());
        }
        assert!(apply_updates(Some(TEMPLATE), "name: [nested]", &updates).is_err());
    }

    #[test]
    fn stale_source_revision_is_rejected_without_replacing_the_caller_revision() {
        let request = request(json!({"form_updates": {"name": "After"}}));
        check_last_revision(None, Some(42), request.edit.last_revision_id).unwrap();
        let error = check_last_revision(None, Some(43), request.edit.last_revision_id)
            .unwrap_err();
        let error = error.frame().error().downcast_ref::<Error>().unwrap();
        assert!(matches!(error.error_type, ErrorType::NotLatestRevisionId));
        assert_eq!(request.edit.last_revision_id, 42);
    }

    #[test]
    fn rejects_invalid_updates_instead_of_changing_the_record() {
        for updates in [
            json!({"missing": "new"}),
            json!({"fixed": "changed"}),
            json!({"kind": "invalid"}),
            json!({"name": ["nested"]}),
        ] {
            let request = request(json!({"form_updates": updates}));
            assert!(
                apply_updates(
                    Some(TEMPLATE),
                    SOURCE,
                    request.form_updates.to_option().unwrap()
                )
                .is_err()
            );
        }
    }
}
