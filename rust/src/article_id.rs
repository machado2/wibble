use crate::entities::content;

pub fn canonical_article_id(raw: &str) -> String {
    raw.trim().to_string()
}

pub fn normalize_content_model(mut article: content::Model) -> content::Model {
    article.id = canonical_article_id(&article.id);
    article
}

pub fn normalize_optional_content_model(article: Option<content::Model>) -> Option<content::Model> {
    article.map(normalize_content_model)
}
