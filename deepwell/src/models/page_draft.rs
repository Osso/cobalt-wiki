use sea_orm::entity::prelude::*;

#[derive(Clone, Debug, PartialEq, DeriveEntityModel, Eq)]
#[sea_orm(table_name = "page_draft")]
pub struct Model {
    #[sea_orm(primary_key, auto_increment = false)]
    pub site_id: i64,
    #[sea_orm(primary_key, auto_increment = false, column_type = "Text")]
    pub slug: String,
    #[sea_orm(column_type = "Text")]
    pub title: String,
    #[sea_orm(column_type = "Text")]
    pub wikitext: String,
    pub saved_by_user_id: Option<i64>,
    pub updated_at: TimeDateTimeWithTimeZone,
}

#[derive(Copy, Clone, Debug, EnumIter, DeriveRelation)]
pub enum Relation {}

impl ActiveModelBehavior for ActiveModel {}
