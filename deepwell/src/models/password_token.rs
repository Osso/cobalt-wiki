use sea_orm::entity::prelude::*;

#[derive(Clone, Debug, PartialEq, Eq, DeriveEntityModel)]
#[sea_orm(table_name = "password_token")]
pub struct Model {
    #[sea_orm(primary_key)]
    pub token_id: i64,
    pub user_id: i64,
    #[sea_orm(column_type = "VarBinary(StringLen::None)", unique)]
    pub token_hash: Vec<u8>,
    pub created_at: TimeDateTimeWithTimeZone,
    pub expires_at: TimeDateTimeWithTimeZone,
    pub emailed_at: Option<TimeDateTimeWithTimeZone>,
    pub used_at: Option<TimeDateTimeWithTimeZone>,
}

#[derive(Copy, Clone, Debug, EnumIter, DeriveRelation)]
pub enum Relation {
    #[sea_orm(
        belongs_to = "super::user::Entity",
        from = "Column::UserId",
        to = "super::user::Column::UserId",
        on_update = "NoAction",
        on_delete = "NoAction"
    )]
    User,
}

impl Related<super::user::Entity> for Entity {
    fn to() -> RelationDef {
        Relation::User.def()
    }
}

impl ActiveModelBehavior for ActiveModel {}
