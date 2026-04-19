use crate::auth::AuthUser;
use crate::entities::content;

pub fn can_edit_article(auth_user: &AuthUser, _article: &content::Model) -> bool {
    auth_user.is_admin()
}

pub fn can_toggle_publish(auth_user: &AuthUser, article: &content::Model) -> bool {
    if auth_user.is_admin() {
        return true;
    }

    article.author_email.as_deref() == Some(&auth_user.email)
}
