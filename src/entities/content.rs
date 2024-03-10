//! `SeaORM` Entity. Generated by sea-orm-codegen 0.12.15

use sea_orm::entity::prelude::*;

#[derive(Clone, Debug, PartialEq, DeriveEntityModel)]
#[sea_orm(table_name = "content")]
pub struct Model {
    #[sea_orm(primary_key, auto_increment = false)]
    pub id: String,
    #[sea_orm(unique)]
    pub slug: String,
    #[sea_orm(column_type = "custom(\"MEDIUMTEXT\")", nullable)]
    pub content: Option<String>,
    pub created_at: DateTime,
    pub generating: i8,
    pub generation_started_at: Option<DateTime>,
    pub generation_finished_at: Option<DateTime>,
    pub flagged: i8,
    pub model: String,
    pub prompt_version: i32,
    pub fail_count: i32,
    pub description: String,
    pub image_id: Option<String>,
    pub title: String,
    pub user_input: String,
    pub view_count: i32,
    pub image_prompt: Option<String>,
    pub user_email: Option<String>,
    pub votes: i32,
    #[sea_orm(column_type = "Double")]
    pub hot_score: f64,
    pub longview_count: i32,
    pub umami_view_count: i32,
    pub generation_time_ms: Option<i32>,
    pub flarum_id: Option<i32>,
    #[sea_orm(column_type = "custom(\"MEDIUMTEXT\")", nullable)]
    pub markdown: Option<String>,
    pub converted: i8,
    pub lemmy_id: Option<i32>,
    pub last_lemmy_post_attempt: Option<DateTime>,
}

#[derive(Copy, Clone, Debug, EnumIter, DeriveRelation)]
pub enum Relation {
    #[sea_orm(
        belongs_to = "super::content_image::Entity",
        from = "Column::ImageId",
        to = "super::content_image::Column::Id",
        on_update = "Cascade",
        on_delete = "SetNull"
    )]
    ContentImage,
    #[sea_orm(has_many = "super::content_vote::Entity")]
    ContentVote,
}

impl Related<super::content_image::Entity> for Entity {
    fn to() -> RelationDef {
        Relation::ContentImage.def()
    }
}

impl Related<super::content_vote::Entity> for Entity {
    fn to() -> RelationDef {
        Relation::ContentVote.def()
    }
}

impl ActiveModelBehavior for ActiveModel {}
