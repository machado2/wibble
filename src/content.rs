use crate::error::Error;
use crate::permissions::{can_edit_article, can_toggle_publish};
use crate::wibble_request::WibbleRequest;
use axum::response::Html;

mod comments;
mod policy;
mod query;
mod render;

pub use comments::{normalize_comment_body, normalize_comments_page};
pub use policy::{article_accepts_public_interactions, can_view_article};
pub use query::{find_article_by_slug, require_article_by_slug};

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

impl GetContent for WibbleRequest {
    async fn get_content_paged(
        &self,
        slug: &str,
        after_id: Option<String>,
    ) -> Result<Html<String>, Error> {
        let _c = query::find_article_after_id(&self.state.db, slug, after_id).await?;
        Ok(Html("".to_string()))
    }

    async fn get_content(
        &self,
        slug: &str,
        source: Option<&str>,
        comments_page: Option<u64>,
    ) -> Result<Html<String>, Error> {
        let article = match query::load_content_page_article(self, slug).await? {
            query::ContentPageArticle::Ready(article) => article,
            query::ContentPageArticle::Wait(wait_page) => return Ok(wait_page),
        };

        if policy::should_track_top_click(source, self.auth_user.is_some()) {
            query::increment_click_count(&self.state.db, &article.id).await?;
        }
        let interactions_open = article_accepts_public_interactions(&article);
        let comment_page = comments::load_comment_page(
            &self.state.db,
            &article.id,
            comments_page,
            interactions_open,
        )
        .await?;
        let user_vote = query::load_user_vote(
            &self.state.db,
            &article.id,
            self.auth_user.as_ref(),
            interactions_open,
        )
        .await?;

        let public_article = article.published && !article.flagged;
        let image_id = article.image_id.clone().unwrap_or_default();
        let markdown = article
            .markdown
            .as_deref()
            .ok_or(Error::NotFound(Some(format!(
                "Markdown for content {} not found",
                article.id
            ))))?;
        let rendered_body = render::markdown_to_html(&render::strip_leading_description(
            markdown,
            &article.description,
        ));
        let mut template = self.template("content").await;
        template
            .insert("id", &article.id)
            .insert("slug", &article.slug)
            .insert("created_at", &article.created_at.format("%F").to_string())
            .insert("description", &article.description)
            .insert("image_id", &image_id)
            .insert("title", &article.title)
            .insert("body", &rendered_body)
            .insert(
                "can_edit",
                &self
                    .auth_user
                    .as_ref()
                    .is_some_and(|u| can_edit_article(u, &article)),
            )
            .insert(
                "can_publish",
                &self
                    .auth_user
                    .as_ref()
                    .is_some_and(|u| can_toggle_publish(u, &article)),
            )
            .insert("is_published", &article.published)
            .insert("vote_score", &article.votes)
            .insert("voting_open", &interactions_open)
            .insert("can_vote", &(interactions_open && self.auth_user.is_some()))
            .insert("user_vote", &user_vote)
            .insert("comments", &comment_page.comments)
            .insert("comment_count", &comment_page.comment_count)
            .insert("comments_open", &interactions_open)
            .insert(
                "can_comment",
                &(interactions_open && self.auth_user.is_some()),
            )
            .insert("comment_pager", &comment_page.pager);
        if !public_article {
            template.insert("robots", "noindex,nofollow");
        }
        template.render()
    }
}
