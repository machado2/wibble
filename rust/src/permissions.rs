use crate::auth::AuthUser;
use crate::entities::content;

pub fn can_edit_article(auth_user: &AuthUser, article: &content::Model) -> bool {
    auth_user.is_admin() || article.author_email.as_deref() == Some(&auth_user.email)
}

pub fn can_toggle_publish(auth_user: &AuthUser, article: &content::Model) -> bool {
    if auth_user.is_admin() {
        return true;
    }

    article.author_email.as_deref() == Some(&auth_user.email)
}

#[cfg(test)]
mod tests {
    use super::{can_edit_article, can_toggle_publish};
    use crate::auth::AuthUser;
    use crate::entities::content;

    fn sample_user(email: &str) -> AuthUser {
        AuthUser {
            sub: "sub".to_string(),
            email: email.to_string(),
            name: "User".to_string(),
            picture: None,
        }
    }

    fn sample_article(author_email: Option<&str>) -> content::Model {
        content::Model {
            id: "id".to_string(),
            slug: "slug".to_string(),
            content: None,
            created_at: chrono::NaiveDate::from_ymd_opt(2026, 4, 19)
                .unwrap()
                .and_hms_opt(12, 0, 0)
                .unwrap(),
            generating: false,
            generation_started_at: None,
            generation_finished_at: None,
            flagged: false,
            model: "model".to_string(),
            prompt_version: 1,
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
            markdown: Some("body".to_string()),
            converted: false,
            longview_count: 0,
            impression_count: 0,
            click_count: 0,
            author_email: author_email.map(str::to_string),
            published: false,
            recovered_from_dead_link: false,
        }
    }

    #[test]
    fn authors_can_edit_their_own_articles() {
        let article = sample_article(Some("author@example.com"));

        assert!(can_edit_article(
            &sample_user("author@example.com"),
            &article
        ));
        assert!(!can_edit_article(
            &sample_user("reader@example.com"),
            &article
        ));
    }

    #[test]
    fn authors_can_toggle_publish_on_their_own_articles() {
        let article = sample_article(Some("author@example.com"));

        assert!(can_toggle_publish(
            &sample_user("author@example.com"),
            &article
        ));
        assert!(!can_toggle_publish(
            &sample_user("reader@example.com"),
            &article
        ));
    }
}
