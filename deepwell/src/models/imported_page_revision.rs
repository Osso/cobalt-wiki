//! Read-only Wikidot revision history acquired from the source site.

use sea_orm::entity::prelude::*;
use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, PartialEq, DeriveEntityModel, Eq, Serialize, Deserialize)]
#[sea_orm(table_name = "imported_page_revision")]
pub struct Model {
    #[sea_orm(primary_key, auto_increment = false)]
    pub site_id: i64,
    #[sea_orm(primary_key, auto_increment = false)]
    pub page_id: i64,
    pub source_page_id: i64,
    #[sea_orm(primary_key, auto_increment = false)]
    pub source_revision_id: i64,
    pub source_revision_number: i32,
    pub source_author_id: Option<i64>,
    #[serde(with = "time::serde::rfc3339")]
    pub source_created_at: TimeDateTimeWithTimeZone,
    #[sea_orm(column_type = "Text")]
    pub source_comments: String,
    pub source_flags: Vec<String>,
    #[sea_orm(column_type = "Text", nullable)]
    pub source_title: Option<String>,
    #[sea_orm(column_type = "Text", nullable)]
    pub source_slug: Option<String>,
    pub source_tags: Option<Vec<String>>,
    #[sea_orm(column_type = "VarBinary(StringLen::None)")]
    pub wikitext_hash: Vec<u8>,
    #[sea_orm(column_type = "Text")]
    pub raw_source_html: String,
    #[serde(with = "time::serde::rfc3339")]
    pub acquired_at: TimeDateTimeWithTimeZone,
    #[sea_orm(column_type = "Text")]
    pub representation: String,
}

#[derive(Copy, Clone, Debug, EnumIter, DeriveRelation)]
pub enum Relation {
    #[sea_orm(
        belongs_to = "super::page::Entity",
        from = "Column::PageId",
        to = "super::page::Column::PageId",
        on_update = "NoAction",
        on_delete = "NoAction"
    )]
    Page,
    #[sea_orm(
        belongs_to = "super::site::Entity",
        from = "Column::SiteId",
        to = "super::site::Column::SiteId",
        on_update = "NoAction",
        on_delete = "NoAction"
    )]
    Site,
    #[sea_orm(
        belongs_to = "super::text::Entity",
        from = "Column::WikitextHash",
        to = "super::text::Column::Hash",
        on_update = "NoAction",
        on_delete = "NoAction"
    )]
    Text,
}

impl Related<super::page::Entity> for Entity {
    fn to() -> RelationDef {
        Relation::Page.def()
    }
}

impl Related<super::site::Entity> for Entity {
    fn to() -> RelationDef {
        Relation::Site.def()
    }
}

impl Related<super::text::Entity> for Entity {
    fn to() -> RelationDef {
        Relation::Text.def()
    }
}

impl ActiveModelBehavior for ActiveModel {}
