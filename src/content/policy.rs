use crate::auth::AuthUser;
use crate::entities::content;

pub fn should_track_top_click(source: Option<&str>, is_logged_in: bool) -> bool {
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

#[cfg(test)]
mod tests {
    use super::{article_accepts_public_interactions, can_view_article, should_track_top_click};
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
