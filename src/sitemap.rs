use std::env;

use axum::extract::State;
use axum::http::header;
use axum::response::IntoResponse;
use sea_orm::prelude::DateTime;
use sea_orm::{ColumnTrait, EntityTrait, QueryFilter, QueryOrder, QuerySelect};

use crate::app_state::AppState;
use crate::entities::{content, prelude::*};
use crate::error::Error;

fn site_url() -> String {
    env::var("SITE_URL")
        .ok()
        .map(|url| url.trim().trim_end_matches('/').to_string())
        .filter(|url| !url.is_empty())
        .unwrap_or_else(|| "https://wibble.news".to_string())
}

fn xml_escape(value: &str) -> String {
    value
        .replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
        .replace('"', "&quot;")
        .replace('\'', "&apos;")
}

pub async fn get_sitemap(State(state): State<AppState>) -> Result<impl IntoResponse, Error> {
    let items: Vec<(String, DateTime)> = Content::find()
        .filter(content::Column::Flagged.eq(false))
        .filter(content::Column::Generating.eq(false))
        .select_only()
        .column(content::Column::Slug)
        .column(content::Column::CreatedAt)
        .order_by_desc(content::Column::CreatedAt)
        .into_tuple()
        .all(&state.db)
        .await
        .map_err(|e| Error::Database(format!("Failed to build sitemap query: {}", e)))?;

    let site_url = site_url();
    let mut xml = String::with_capacity(items.len() * 128 + 512);
    xml.push_str("<?xml version=\"1.0\" encoding=\"UTF-8\"?>");
    xml.push_str("<urlset xmlns=\"http://www.sitemaps.org/schemas/sitemap/0.9\">");
    xml.push_str("<url><loc>");
    xml.push_str(&xml_escape(&format!("{}/", site_url)));
    xml.push_str("</loc></url>");

    for (slug, created_at) in items {
        let loc = format!("{}/content/{}", site_url, slug);
        xml.push_str("<url><loc>");
        xml.push_str(&xml_escape(&loc));
        xml.push_str("</loc><lastmod>");
        xml.push_str(&created_at.format("%F").to_string());
        xml.push_str("</lastmod></url>");
    }
    xml.push_str("</urlset>");

    Ok((
        [(header::CONTENT_TYPE, "application/xml; charset=utf-8")],
        xml,
    ))
}

pub async fn get_robots_txt() -> impl IntoResponse {
    let site_url = site_url();
    let robots = format!(
        "User-agent: *\nAllow: /\nSitemap: {}/sitemap.xml\n",
        site_url
    );
    (
        [(header::CONTENT_TYPE, "text/plain; charset=utf-8")],
        robots,
    )
}
