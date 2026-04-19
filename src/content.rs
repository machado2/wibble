use std::sync::OnceLock;

use axum::response::Html;
use markdown::{to_html, to_html_with_options, Options};
use regex::Regex;
use sea_orm::sea_query::Expr;
use sea_orm::{ColumnTrait, EntityTrait, PaginatorTrait, QueryFilter, QueryOrder, QuerySelect};
use serde::Serialize;
use tracing::{event, warn, Level};

use crate::auth::AuthUser;
use crate::create::{render_wait_page, start_recover_article_for_slug};
use crate::tasklist::TaskResult;
use crate::wibble_request::WibbleRequest;
use crate::{
    entities::{content, content_comment, content_image, prelude::*},
    error::Error,
};

#[allow(async_fn_in_trait)]
pub trait GetContent {
    async fn get_content(
        &self,
        slug: &str,
        source: Option<&str>,
        comments_page: Option<u64>,
    ) -> Result<Html<String>, Error>;
    async fn get_content_paged(
        &self,
        slug: &str,
        after_id: Option<String>,
    ) -> Result<Html<String>, Error>;
}

fn article_image_regex() -> &'static Regex {
    static ARTICLE_IMAGE_REGEX: OnceLock<Regex> = OnceLock::new();
    ARTICLE_IMAGE_REGEX.get_or_init(|| {
        Regex::new(r#"<img src="(/image/([^"/?#]+))"([^>]*) />"#)
            .expect("article image regex must compile")
    })
}

fn link_article_images(html: &str) -> String {
    article_image_regex()
        .replace_all(html, |caps: &regex::Captures<'_>| {
            format!(
                r#"<a href="/image_info/{id}" class="article-image-link">{img}</a>"#,
                id = &caps[2],
                img = &caps[0],
            )
        })
        .into_owned()
}

fn markdown_to_html(markdown_str: &str) -> String {
    let html = to_html_with_options(markdown_str, &Options::gfm())
        .unwrap_or_else(|_| to_html(markdown_str));
    link_article_images(&html)
}

fn strip_leading_description(markdown: &str, description: &str) -> String {
    let markdown = markdown.trim();
    let description = description.trim();
    if description.is_empty() {
        return markdown.to_string();
    }

    let mut parts = markdown.splitn(2, "\n\n");
    let first_block = parts.next().unwrap_or("").trim();
    if first_block == description {
        parts.next().unwrap_or("").trim().to_string()
    } else {
        markdown.to_string()
    }
}

fn should_track_top_click(source: Option<&str>, is_logged_in: bool) -> bool {
    source == Some("top") && is_logged_in
}

pub fn article_accepts_public_interactions(article: &content::Model) -> bool {
    article.published && !article.flagged && !article.generating
}

pub fn can_view_article(auth_user: Option<&AuthUser>, article: &content::Model) -> bool {
    if article.published && !article.flagged {
        return true;
    }

    auth_user
        .is_some_and(|user| user.is_admin() || article.author_email.as_deref() == Some(&user.email))
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

#[derive(Serialize)]
struct CommentView {
    user_name: String,
    body: String,
    created_at: String,
}

#[derive(Serialize)]
struct CommentPager {
    current_page: u64,
    total_pages: u64,
    has_prev: bool,
    has_next: bool,
    prev_page: u64,
    next_page: u64,
}

impl GetContent for WibbleRequest {
    async fn get_content_paged(
        &self,
        slug: &str,
        after_id: Option<String>,
    ) -> Result<Html<String>, Error> {
        let state = &self.state;
        let db = &state.db;
        let _c = Content::find()
            .filter(content::Column::Slug.contains(slug))
            .filter(content::Column::Id.gt(after_id.unwrap_or_default()))
            .one(db)
            .await
            .map_err(|e| Error::Database(format!("Dataabase error reading content: {}", e)))?
            .ok_or(Error::NotFound(Some(format!(
                "Content with slug {} not found",
                slug
            ))))?;
        Ok(Html("".to_string()))
    }

    async fn get_content(
        &self,
        slug: &str,
        source: Option<&str>,
        comments_page: Option<u64>,
    ) -> Result<Html<String>, Error> {
        let state = &self.state;
        let db = &state.db;
        let mut c = Content::find()
            .filter(content::Column::Slug.eq(slug))
            .one(db)
            .await
            .map_err(|e| Error::Database(format!("Dataabase error reading content: {}", e)))?;

        if c.is_none() {
            if let Err(e) = start_recover_article_for_slug(state.clone(), slug.to_string()).await {
                warn!(slug = %slug, error = %e, "Failed to start dead-link recovery");
            }
            c = Content::find()
                .filter(content::Column::Slug.eq(slug))
                .one(db)
                .await
                .map_err(|e| Error::Database(format!("Dataabase error reading content: {}", e)))?;
        }

        let mut c = c.ok_or(Error::NotFound(Some(format!(
            "Content with slug {} not found",
            slug
        ))))?;

        if !can_view_article(self.auth_user.as_ref(), &c) {
            return Err(Error::NotFound(Some(format!(
                "Content with slug {} not found",
                slug
            ))));
        }

        if c.generating {
            let task_processing =
                matches!(state.task_list.get(&c.id).await, Ok(TaskResult::Processing));
            if state.is_generation_active(&c.id).await || task_processing {
                event!(
                    Level::INFO,
                    slug = %slug,
                    article_id = %c.id,
                    "Serving wait page for active generation"
                );
                return render_wait_page(self, &c.id).await;
            }

            if c.markdown.is_some() {
                warn!(
                    slug = %slug,
                    article_id = %c.id,
                    "Found stale generating row with markdown; flipping generating=false"
                );
                Content::update_many()
                    .filter(content::Column::Id.eq(c.id.clone()))
                    .col_expr(content::Column::Generating, Expr::value(false))
                    .exec(db)
                    .await
                    .map_err(|e| {
                        Error::Database(format!("Failed to clear stale generating flag: {}", e))
                    })?;
                c.generating = false;
            } else {
                warn!(
                    slug = %slug,
                    article_id = %c.id,
                    "Found stale generating row with no in-memory active task; removing and retrying recovery"
                );
                ContentImage::delete_many()
                    .filter(content_image::Column::ContentId.eq(c.id.clone()))
                    .exec(db)
                    .await
                    .map_err(|e| {
                        Error::Database(format!("Failed to delete stale content images: {}", e))
                    })?;
                Content::delete_by_id(c.id.clone())
                    .exec(db)
                    .await
                    .map_err(|e| {
                        Error::Database(format!("Failed to delete stale content row: {}", e))
                    })?;

                if let Err(e) =
                    start_recover_article_for_slug(state.clone(), slug.to_string()).await
                {
                    warn!(slug = %slug, error = %e, "Failed to restart dead-link recovery");
                }
                c = Content::find()
                    .filter(content::Column::Slug.eq(slug))
                    .one(db)
                    .await
                    .map_err(|e| {
                        Error::Database(format!("Dataabase error reading content: {}", e))
                    })?
                    .ok_or(Error::NotFound(Some(format!(
                        "Content with slug {} not found",
                        slug
                    ))))?;

                if !can_view_article(self.auth_user.as_ref(), &c) {
                    return Err(Error::NotFound(Some(format!(
                        "Content with slug {} not found",
                        slug
                    ))));
                }
            }

            if c.generating {
                return render_wait_page(self, &c.id).await;
            }
        }

        if should_track_top_click(source, self.auth_user.is_some()) {
            Content::update_many()
                .filter(content::Column::Id.eq(c.id.clone()))
                .col_expr(
                    content::Column::ClickCount,
                    Expr::col(content::Column::ClickCount).add(1),
                )
                .exec(db)
                .await
                .map_err(|e| Error::Database(format!("Error updating click count: {}", e)))?;
        }
        let interactions_open = article_accepts_public_interactions(&c);
        let comment_page = normalize_comments_page(comments_page);
        let (comments, comment_count, comment_pager) = if interactions_open {
            const COMMENTS_PER_PAGE: u64 = 50;
            let comment_count = ContentComment::find()
                .filter(content_comment::Column::ContentId.eq(c.id.clone()))
                .count(db)
                .await
                .map_err(|e| Error::Database(format!("Error counting comments: {}", e)))?;
            let total_pages = comment_count.max(1).div_ceil(COMMENTS_PER_PAGE);
            let current_page = comment_page.min(total_pages);
            let offset = (current_page - 1) * COMMENTS_PER_PAGE;
            let mut comments = ContentComment::find()
                .filter(content_comment::Column::ContentId.eq(c.id.clone()))
                .order_by_desc(content_comment::Column::CreatedAt)
                .offset(offset)
                .limit(COMMENTS_PER_PAGE)
                .all(db)
                .await
                .map_err(|e| Error::Database(format!("Error loading comments: {}", e)))?;
            comments.reverse();
            let comments = comments
                .into_iter()
                .map(|comment| CommentView {
                    user_name: comment.user_name,
                    body: comment.body,
                    created_at: comment.created_at.format("%F %R").to_string(),
                })
                .collect::<Vec<_>>();
            let pager = CommentPager {
                current_page,
                total_pages,
                has_prev: current_page > 1,
                has_next: current_page < total_pages,
                prev_page: current_page.saturating_sub(1).max(1),
                next_page: current_page + 1,
            };
            (comments, comment_count, pager)
        } else {
            (
                Vec::new(),
                0,
                CommentPager {
                    current_page: 1,
                    total_pages: 1,
                    has_prev: false,
                    has_next: false,
                    prev_page: 1,
                    next_page: 1,
                },
            )
        };
        let user_vote = if interactions_open {
            if let Some(auth_user) = self.auth_user.as_ref() {
                ContentVote::find_by_id((c.id.clone(), auth_user.email.clone()))
                    .one(db)
                    .await
                    .map_err(|e| Error::Database(format!("Error loading vote: {}", e)))?
                    .map(|vote| {
                        if vote.downvote {
                            "down".to_string()
                        } else {
                            "up".to_string()
                        }
                    })
                    .unwrap_or_default()
            } else {
                String::new()
            }
        } else {
            String::new()
        };

        let public_article = c.published && !c.flagged;
        let markdown = c.markdown.as_deref().ok_or(Error::NotFound(Some(format!(
            "Markdown for content {} not found",
            c.id
        ))))?;
        let rendered_body = markdown_to_html(&strip_leading_description(markdown, &c.description));
        let mut template = self.template("content").await;
        template
            .insert("id", &c.id)
            .insert("slug", &c.slug)
            .insert("created_at", &c.created_at.format("%F").to_string())
            .insert("description", &c.description)
            .insert("image_id", &c.image_id.unwrap_or_default())
            .insert("title", &c.title)
            .insert("body", &rendered_body)
            .insert(
                "can_edit",
                &self.auth_user.as_ref().is_some_and(|u| u.is_admin()),
            )
            .insert(
                "can_publish",
                &self
                    .auth_user
                    .as_ref()
                    .is_some_and(|u| u.is_admin() || c.author_email.as_deref() == Some(&u.email)),
            )
            .insert("is_published", &c.published)
            .insert("vote_score", &c.votes)
            .insert("voting_open", &interactions_open)
            .insert("can_vote", &(interactions_open && self.auth_user.is_some()))
            .insert("user_vote", &user_vote)
            .insert("comments", &comments)
            .insert("comment_count", &comment_count)
            .insert("comments_open", &interactions_open)
            .insert(
                "can_comment",
                &(interactions_open && self.auth_user.is_some()),
            )
            .insert("comment_pager", &comment_pager);
        if !public_article {
            template.insert("robots", "noindex,nofollow");
        }
        template.render()
    }
}

#[cfg(test)]
mod tests {
    use super::{
        article_accepts_public_interactions, can_view_article, markdown_to_html,
        normalize_comment_body, normalize_comments_page, should_track_top_click,
        strip_leading_description,
    };
    use crate::auth::AuthUser;
    use crate::entities::content;

    fn sample_article() -> content::Model {
        content::Model {
            id: "id".to_string(),
            slug: "slug".to_string(),
            content: None,
            created_at: chrono::NaiveDate::from_ymd_opt(2026, 4, 18)
                .unwrap()
                .and_hms_opt(10, 0, 0)
                .unwrap(),
            generating: false,
            generation_started_at: None,
            generation_finished_at: None,
            flagged: false,
            model: "model".to_string(),
            prompt_version: 0,
            fail_count: 0,
            description: "desc".to_string(),
            image_id: None,
            title: "title".to_string(),
            user_input: "input".to_string(),
            image_prompt: None,
            user_email: None,
            votes: 0,
            hot_score: 0.0,
            generation_time_ms: None,
            flarum_id: None,
            markdown: None,
            converted: false,
            longview_count: 0,
            impression_count: 0,
            click_count: 0,
            author_email: None,
            published: true,
            recovered_from_dead_link: false,
        }
    }

    fn sample_user(email: &str) -> AuthUser {
        AuthUser {
            sub: "sub".to_string(),
            email: email.to_string(),
            name: "User".to_string(),
            picture: None,
        }
    }

    #[test]
    fn tracks_top_clicks_for_logged_in_users_only() {
        assert!(should_track_top_click(Some("top"), true));
        assert!(!should_track_top_click(Some("top"), false));
        assert!(!should_track_top_click(None, true));
        assert!(!should_track_top_click(Some("other"), true));
    }

    #[test]
    fn normalizes_comment_body() {
        assert_eq!(
            normalize_comment_body("  hello world  ").unwrap(),
            "hello world"
        );
        assert!(normalize_comment_body("   ").is_err());
    }

    #[test]
    fn only_published_finished_unflagged_articles_accept_public_interactions() {
        let base = sample_article();
        assert!(article_accepts_public_interactions(&base));

        let mut draft = sample_article();
        draft.published = false;
        assert!(!article_accepts_public_interactions(&draft));

        let mut generating = sample_article();
        generating.generating = true;
        assert!(!article_accepts_public_interactions(&generating));

        let mut flagged = sample_article();
        flagged.flagged = true;
        assert!(!article_accepts_public_interactions(&flagged));
    }

    #[test]
    fn normalizes_comment_page_numbers() {
        assert_eq!(normalize_comments_page(None), 1);
        assert_eq!(normalize_comments_page(Some(0)), 1);
        assert_eq!(normalize_comments_page(Some(3)), 3);
    }

    #[test]
    fn markdown_rendering_escapes_raw_html_and_wraps_article_images() {
        let rendered = markdown_to_html(
            r#"<script>alert(1)</script>

![Alt text](/image/abc-123 "Prompt")
"#,
        );

        assert!(!rendered.contains("<script>"));
        assert!(rendered.contains("&lt;script&gt;alert(1)&lt;/script&gt;"));
        assert!(rendered.contains(r#"href="/image_info/abc-123""#));
        assert!(rendered.contains(r#"src="/image/abc-123""#));
    }

    #[test]
    fn strips_duplicate_standfirst_from_article_body() {
        let markdown = "Opening paragraph.\n\n## Section\n\nMore detail.";
        assert_eq!(
            strip_leading_description(markdown, "Opening paragraph."),
            "## Section\n\nMore detail."
        );
        assert_eq!(
            strip_leading_description(markdown, "Something else"),
            markdown
        );
    }

    #[test]
    fn unpublished_articles_are_visible_only_to_author_or_admin() {
        let mut article = sample_article();
        article.published = false;
        article.author_email = Some("author@example.com".to_string());

        assert!(!can_view_article(None, &article));
        assert!(!can_view_article(
            Some(&sample_user("reader@example.com")),
            &article
        ));
        assert!(can_view_article(
            Some(&sample_user("author@example.com")),
            &article
        ));
    }
}
