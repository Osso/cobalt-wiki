//! Transaction-scoped deletion of shared drafts after successful publication.

use crate::error::prelude::*;
use crate::models::page_draft::Entity as PageDraft;
use sea_orm::{DatabaseTransaction, EntityTrait};

#[derive(Debug)]
pub struct PageDraftService;

impl PageDraftService {
    pub async fn delete_for_target(
        transaction: &DatabaseTransaction,
        site_id: i64,
        slug: &str,
    ) -> Result<()> {
        PageDraft::delete_by_id((site_id, slug.to_owned()))
            .exec(transaction)
            .await
            .or_raise(|| Error::new("failed to delete page draft", ErrorType::Page))?;
        Ok(())
    }
}
