use axum::response::Html;
use markdown::mdast::Node;
use markdown::{to_html, to_mdast, ParseOptions};
use sea_orm::sea_query::Expr;
use sea_orm::{ColumnTrait, EntityTrait, QueryFilter};

use crate::wibble_request::WibbleRequest;
use crate::{
    entities::{content, prelude::*},
    error::Error,
};

#[allow(async_fn_in_trait)]
pub trait GetContent {
    async fn get_content(&self, slug: &str, source: Option<&str>) -> Result<Html<String>, Error>;
    async fn get_content_paged(
        &self,
        slug: &str,
        after_id: Option<String>,
    ) -> Result<Html<String>, Error>;
}

fn preprocess_markdown_node(node: &mut Node) {
    if let Node::Image(i) = node {
        let id = i.url.split('/').next_back().unwrap_or_default();
        let link_url = format!("/image_info/{}", id);
        let link_node = Node::Link(markdown::mdast::Link {
            url: link_url,
            title: None,
            children: vec![node.clone()],
            position: None,
        });
        *node = link_node;
    }
}

fn preprocess_markdown_tree(node: &mut Node) {
    if let Some(children) = node.children_mut() {
        for child in children {
            preprocess_markdown_tree(child);
            preprocess_markdown_node(child);
        }
    }
}

fn mdast_to_html_inner(node: &Node, output: &mut String) {
    let push_children = |output: &mut String| {
        if let Some(children) = node.children() {
            for child in children {
                mdast_to_html_inner(child, &mut *output);
            }
        }
    };
    match node {
        Node::Root(_) => {
            push_children(output);
        }
        Node::Blockquote(_) => {
            output.push_str("<blockquote>");
            push_children(output);
            output.push_str("</blockquote>");
        }
        Node::List(l) => {
            let tag = if l.ordered { "ol" } else { "ul" };
            output.push('<');
            output.push_str(tag);
            output.push('>');
            push_children(output);
            output.push_str("</");
            output.push_str(tag);
            output.push('>');
        }
        Node::Break(_) => {
            output.push_str("<br>");
        }
        Node::InlineCode(ic) => {
            output.push_str("<code>");
            output.push_str(&ic.value);
            output.push_str("</code>");
        }
        Node::InlineMath(im) => {
            output.push_str("<span>");
            output.push_str(&im.value);
            output.push_str("</span>");
        }
        Node::Delete(_) => {
            output.push_str("<del>");
            push_children(output);
            output.push_str("</del>");
        }
        Node::Emphasis(_) => {
            output.push_str("<em>");
            push_children(output);
            output.push_str("</em>");
        }
        Node::Html(h) => {
            output.push_str(&h.value);
        }
        Node::Image(i) => {
            output.push_str("<img src=\"");
            output.push_str(&i.url);
            output.push_str("\" alt=\"");
            output.push_str(&i.alt);
            output.push('"');
            if let Some(title) = &i.title {
                output.push_str(" title=\"");
                output.push_str(title);
                output.push('"');
            }
            output.push_str("\">");
        }
        Node::Link(l) => {
            output.push_str("<a href=\"");
            output.push_str(&l.url);
            output.push('"');
            if let Some(title) = &l.title {
                output.push_str(" title=\"");
                output.push_str(title);
                output.push('"');
            }
            output.push_str("\">");
            push_children(output);
            output.push_str("</a>");
        }
        Node::Strong(_) => {
            output.push_str("<strong>");
            push_children(output);
            output.push_str("</strong>");
        }
        Node::Text(t) => {
            output.push_str(&t.value);
        }
        Node::Code(c) => {
            output.push_str("<pre><code>");
            output.push_str(&c.value);
            output.push_str("</code></pre>");
        }
        Node::Math(m) => {
            output.push_str("<span>");
            output.push_str(&m.value);
            output.push_str("</span>");
        }
        Node::Heading(h) => {
            output.push_str("<h");
            output.push_str(&h.depth.to_string());
            output.push('>');
            push_children(output);
            output.push_str("</h");
            output.push_str(&h.depth.to_string());
            output.push('>');
        }
        Node::Table(_) => {
            output.push_str("<table>");
            push_children(output);
            output.push_str("</table>");
        }
        Node::ThematicBreak(_) => {
            output.push_str("<hr>");
        }
        Node::TableRow(_) => {
            output.push_str("<tr>");
            push_children(output);
            output.push_str("</tr>");
        }
        Node::TableCell(_) => {
            output.push_str("<td>");
            push_children(output);
            output.push_str("</td>");
        }
        Node::ListItem(_) => {
            output.push_str("<li>");
            push_children(output);
            output.push_str("</li>");
        }
        Node::Paragraph(_) => {
            output.push_str("<p>");
            push_children(output);
            output.push_str("</p>");
        }
        _ => {
            push_children(output);
        }
    }
}

fn mdast_to_html(node: &Node) -> String {
    let mut output = String::new();
    mdast_to_html_inner(node, &mut output);
    output
}

fn markdown_to_html(markdown_str: &str) -> String {
    let options = ParseOptions::default();
    if let Ok(mut node) = to_mdast(markdown_str, &options) {
        preprocess_markdown_tree(&mut node);
        mdast_to_html(&node)
    } else {
        to_html(markdown_str)
    }
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
            .ok_or(Error::NotFound)?;
        Ok(Html("".to_string()))
    }

    async fn get_content(&self, slug: &str, source: Option<&str>) -> Result<Html<String>, Error> {
        let state = &self.state;
        let db = &state.db;
        let c = Content::find()
            .filter(content::Column::Slug.contains(slug))
            .one(db)
            .await
            .map_err(|e| Error::Database(format!("Dataabase error reading content: {}", e)))?
            .ok_or(Error::NotFound)?;
        if source == Some("top") {
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
        self.template("content")
            .await
            .insert("id", &c.id)
            .insert("slug", &c.slug)
            .insert("created_at", &c.created_at.format("%F").to_string())
            .insert("description", &c.description)
            .insert("image_id", &c.image_id.unwrap_or_default())
            .insert("title", &c.title)
            .insert(
                "body",
                &markdown_to_html(&c.markdown.ok_or(Error::NotFound)?),
            )
            .render()
    }
}
