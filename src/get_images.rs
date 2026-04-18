use axum::extract::Query;
use axum::response::Html;
use sea_orm::prelude::*;
use sea_orm::{ColumnTrait, QueryFilter, QueryOrder, QuerySelect};
use serde::{Deserialize, Serialize};

use crate::entities::prelude::*;
use crate::entities::{content, content_image};
use crate::error::Error;
use crate::image_status::IMAGE_STATUS_COMPLETED;
use crate::wibble_request::WibbleRequest;

const IMAGES_PER_PAGE: u64 = 24;

#[derive(Deserialize)]
pub struct GetImagesParams {
    #[serde(alias = "afterId")]
    after_id: Option<String>,
}

#[derive(Serialize)]
struct ImageGalleryItem {
    id: String,
    alt_text: String,
    article_slug: String,
    article_title: String,
}

#[derive(Default)]
struct ImageGalleryPage {
    items: Vec<ImageGalleryItem>,
    next_after_id: Option<String>,
}

fn build_next_page_url(after_id: Option<&str>) -> Option<String> {
    after_id.map(|after_id| format!("?afterId={}", after_id))
}

async fn get_gallery_page(
    db: &DatabaseConnection,
    after_id: Option<String>,
) -> Result<ImageGalleryPage, Error> {
    let cursor = match after_id {
        Some(after_id) => ContentImage::find_by_id(after_id)
            .one(db)
            .await
            .map_err(|e| Error::Database(format!("Error loading image cursor: {}", e)))?,
        None => None,
    };

    let mut images = ContentImage::find()
        .find_also_related(Content)
        .filter(content_image::Column::Status.eq(IMAGE_STATUS_COMPLETED))
        .filter(content_image::Column::Flagged.eq(false))
        .filter(content::Column::Published.eq(true))
        .filter(content::Column::Flagged.eq(false))
        .filter(content::Column::Generating.eq(false))
        .order_by_desc(content_image::Column::CreatedAt)
        .order_by_desc(content_image::Column::Id)
        .limit(IMAGES_PER_PAGE + 1);

    if let Some(cursor) = cursor {
        images = images.filter(
            content_image::Column::CreatedAt.lt(cursor.created_at).or(
                content_image::Column::CreatedAt
                    .eq(cursor.created_at)
                    .and(content_image::Column::Id.lt(cursor.id)),
            ),
        );
    }

    let mut rows = images
        .all(db)
        .await
        .map_err(|e| Error::Database(format!("Error loading image gallery: {}", e)))?;

    let has_more = rows.len() > IMAGES_PER_PAGE as usize;
    if has_more {
        rows.truncate(IMAGES_PER_PAGE as usize);
    }

    let items = rows
        .into_iter()
        .filter_map(|(image, article)| {
            article.map(|article| ImageGalleryItem {
                id: image.id,
                alt_text: image.alt_text,
                article_slug: article.slug,
                article_title: article.title,
            })
        })
        .collect::<Vec<_>>();

    let next_after_id = if has_more {
        items.last().map(|item| item.id.clone())
    } else {
        None
    };

    Ok(ImageGalleryPage {
        items,
        next_after_id,
    })
}

pub async fn get_images(
    wr: WibbleRequest,
    Query(params): Query<GetImagesParams>,
) -> Result<Html<String>, Error> {
    let gallery_page = get_gallery_page(&wr.state.db, params.after_id.clone()).await?;
    let next_page_url = build_next_page_url(gallery_page.next_after_id.as_deref());
    let mut template = wr.template("images").await;
    template
        .insert("items", &gallery_page.items)
        .insert("next_page_url", &next_page_url)
        .insert("title", "Generated image gallery")
        .insert(
            "description",
            "A browsable gallery of AI-generated images used in Wibble stories.",
        );
    if params.after_id.is_some() {
        template.insert("robots", "noindex,follow");
    } else {
        template.insert("robots", "noindex,nofollow");
    }
    template.render()
}

#[cfg(test)]
mod tests {
    use super::build_next_page_url;

    #[test]
    fn build_next_page_url_uses_public_after_id_parameter() {
        assert_eq!(
            build_next_page_url(Some("abc-123")).as_deref(),
            Some("?afterId=abc-123")
        );
        assert_eq!(build_next_page_url(None), None);
    }
}
