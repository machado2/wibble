#![allow(non_snake_case)]
#![allow(clippy::blocks_in_conditions)]

use crate::entities::{content_image, prelude::*};
use crate::error::Result;
use chrono::TimeDelta;
use json::JsonValue;
use sea_orm::{prelude::*, QueryOrder, QuerySelect};
use crate::get_db;

pub async fn get_next_page_images(
    after_id: Option<String>,
    page_size: Option<u8>,
    search: Option<String>,
    sort: Option<String>,
    period: Option<String>,
) -> Result<String> {
    let page_size = match page_size {
        Some(i) if i < 100 => i,
        _ => 20,
    };

    let sort_column = match sort {
        Some(s) if s == "most_viewed" => content_image::Column::ViewCount,
        _ => content_image::Column::CreatedAt,
    };

    let after_content = match after_id {
        Some(id) => {
            ContentImage::find()
                .filter(content_image::Column::Id.eq(id))
                .one(get_db())
                .await?
        }
        None => None,
    };

    let mut data = ContentImage::find().filter(content_image::Column::Flagged.eq(false));
    data = match &after_content {
        Some(ac) => data
            .filter(
                content_image::Column::Id
                    .ne(ac.id.clone())
                    .and(sort_column.lte(ac.get(sort_column))),
            )
            .filter(
                sort_column
                    .lt(ac.get(sort_column))
                    .or(content_image::Column::Id.lt(ac.id.clone())),
            ),
        None => data,
    };
    data = match search {
        Some(s) => data.filter(content_image::Column::Prompt.contains(s)),
        None => data,
    };
    let days = match period.unwrap_or_default().as_str() {
        "week" => TimeDelta::try_days(7),
        "month" => TimeDelta::try_days(30),
        _ => None,
    };
    if let Some(d) = days {
        data = data.filter(content_image::Column::CreatedAt.gt(chrono::Utc::now().naive_utc() - d));
    }
    let data = data
        .order_by_desc(sort_column)
        .order_by_desc(content_image::Column::Id)
        .limit(page_size as u64)
        .all(get_db())
        .await?;
    let mut list = JsonValue::new_array();
    for c in data {
        let mut content = JsonValue::new_object();
        content["id"] = c.id.into();
        content["alt_text"] = c.alt_text.into();
        content["created_at"] = c.created_at.and_utc().to_rfc3339().into();
        let _ = list.push(content);
    }
    Ok(list.dump())
}
