//! `SeaORM` Entity. Generated by sea-orm-codegen 0.12.15

use sea_orm::entity::prelude::*;

#[derive(Clone, Debug, PartialEq, DeriveEntityModel, Eq)]
#[sea_orm(table_name = "image_data")]
pub struct Model {
    #[sea_orm(primary_key, auto_increment = false)]
    pub id: String,
    #[sea_orm(column_type = "Binary(BlobSize::Medium)")]
    pub jpeg_data: Vec<u8>,
}

#[derive(Copy, Clone, Debug, EnumIter, DeriveRelation)]
pub enum Relation {
    #[sea_orm(
        belongs_to = "super::content_image::Entity",
        from = "Column::Id",
        to = "super::content_image::Column::Id",
        on_update = "Cascade",
        on_delete = "Cascade"
    )]
    ContentImage,
}

impl Related<super::content_image::Entity> for Entity {
    fn to() -> RelationDef {
        Relation::ContentImage.def()
    }
}

impl ActiveModelBehavior for ActiveModel {}
