use sea_orm::entity::prelude::*;

#[derive(Clone, Debug, PartialEq, DeriveEntityModel, Eq)]
#[sea_orm(table_name = "article_job")]
pub struct Model {
    #[sea_orm(primary_key, auto_increment = false)]
    pub id: String,
    pub article_id: Option<String>,
    pub requester_key: String,
    pub requester_tier: String,
    pub author_email: Option<String>,
    #[sea_orm(column_type = "Text")]
    pub prompt: String,
    pub feature_type: String,
    pub phase: String,
    pub status: String,
    #[sea_orm(column_type = "Text", nullable)]
    pub usage_counters: Option<String>,
    #[sea_orm(column_type = "Text", nullable)]
    pub preview_payload: Option<String>,
    #[sea_orm(column_type = "Text", nullable)]
    pub error_summary: Option<String>,
    pub fail_count: i32,
    pub created_at: DateTime,
    pub updated_at: DateTime,
    pub started_at: Option<DateTime>,
    pub finished_at: Option<DateTime>,
}

#[derive(Copy, Clone, Debug, EnumIter, DeriveRelation)]
pub enum Relation {}

impl ActiveModelBehavior for ActiveModel {}
