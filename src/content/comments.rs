use sea_orm::{
    ColumnTrait, DatabaseConnection, EntityTrait, PaginatorTrait, QueryFilter, QueryOrder,
    QuerySelect,
};
use serde::Serialize;

use crate::entities::{content_comment, prelude::*};
use crate::error::Error;

const COMMENTS_PER_PAGE: u64 = 50;

#[derive(Serialize)]
pub struct CommentView {
    user_name: String,
    body: String,
    created_at: String,
}

#[derive(Serialize)]
pub struct CommentPager {
    current_page: u64,
    total_pages: u64,
    has_prev: bool,
    has_next: bool,
    prev_page: u64,
    next_page: u64,
}

pub struct CommentPage {
    pub comments: Vec<CommentView>,
    pub comment_count: u64,
    pub pager: CommentPager,
}

impl CommentPage {
    fn empty() -> Self {
        Self {
            comments: Vec::new(),
            comment_count: 0,
            pager: CommentPager {
                current_page: 1,
                total_pages: 1,
                has_prev: false,
                has_next: false,
                prev_page: 1,
                next_page: 1,
            },
        }
    }
}

pub fn normalize_comments_page(page: Option<u64>) -> u64 {
    page.unwrap_or(1).max(1)
}

pub fn normalize_comment_body(raw: &str) -> Result<String, Error> {
    let body = raw.trim();
    if body.is_empty() {
        return Err(Error::BadRequest("Comment cannot be empty".to_string()));
    }
    if body.chars().count() > 5_000 {
        return Err(Error::BadRequest("Comment is too long".to_string()));
    }
    Ok(body.to_string())
}

pub async fn load_comment_page(
    db: &DatabaseConnection,
    article_id: &str,
    page: Option<u64>,
    enabled: bool,
) -> Result<CommentPage, Error> {
    if !enabled {
        return Ok(CommentPage::empty());
    }

    let current_page = normalize_comments_page(page);
    let comment_count = ContentComment::find()
        .filter(content_comment::Column::ContentId.eq(article_id.to_string()))
        .count(db)
        .await
        .map_err(|e| Error::Database(format!("Error counting comments: {}", e)))?;
    let total_pages = comment_count.max(1).div_ceil(COMMENTS_PER_PAGE);
    let current_page = current_page.min(total_pages);
    let offset = (current_page - 1) * COMMENTS_PER_PAGE;
    let mut comments = ContentComment::find()
        .filter(content_comment::Column::ContentId.eq(article_id.to_string()))
        .order_by_desc(content_comment::Column::CreatedAt)
        .offset(offset)
        .limit(COMMENTS_PER_PAGE)
        .all(db)
        .await
        .map_err(|e| Error::Database(format!("Error loading comments: {}", e)))?;
    comments.reverse();

    Ok(CommentPage {
        comments: comments
            .into_iter()
            .map(|comment| CommentView {
                user_name: comment.user_name,
                body: comment.body,
                created_at: comment.created_at.format("%F %R").to_string(),
            })
            .collect(),
        comment_count,
        pager: CommentPager {
            current_page,
            total_pages,
            has_prev: current_page > 1,
            has_next: current_page < total_pages,
            prev_page: current_page.saturating_sub(1).max(1),
            next_page: current_page + 1,
        },
    })
}

#[cfg(test)]
mod tests {
    use super::{normalize_comment_body, normalize_comments_page};

    #[test]
    fn normalizes_comment_body() {
        assert_eq!(
            normalize_comment_body("  hello world  ").unwrap(),
            "hello world"
        );
        assert!(normalize_comment_body("   ").is_err());
    }

    #[test]
    fn normalizes_comment_page_numbers() {
        assert_eq!(normalize_comments_page(None), 1);
        assert_eq!(normalize_comments_page(Some(0)), 1);
        assert_eq!(normalize_comments_page(Some(3)), 3);
    }
}
