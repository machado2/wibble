use rand::prelude::*;
use sea_orm::prelude::*;
use sea_orm::{ColumnTrait, QueryFilter, QuerySelect};
use serde_json::Value;

use crate::entities::examples;
use crate::entities::prelude::*;
use crate::error::Error;

pub async fn get_examples(db: &DatabaseConnection) -> Result<Vec<(String, String)>, Error> {
    let k = || async {
        let max_id = Examples::find()
            .select_only()
            .column_as(examples::Column::NewId.max(), "max_new_id")
            .into_tuple::<Option<i32>>()
            .one(db)
            .await?
            .flatten();

        if let Some(max_id) = max_id {
            let random_ids: Vec<i32> = (0..3)
                .map(|_| rand::rng().random_range(1..=max_id))
                .collect();
            let examples = Examples::find()
                .filter(examples::Column::NewId.is_in(random_ids.clone()))
                .all(db)
                .await?;

            Ok(examples
                .into_iter()
                .filter_map(|example| {
                    let first_line = example.content.as_deref()?.lines().next().unwrap_or("");
                    let content = example.content.clone().unwrap_or_default();

                    let user_input = if example.user_input.starts_with('{') {
                        let json: Value = serde_json::from_str(&example.user_input).ok()?;
                        json["suggestion"].as_str().map(String::from)
                    } else {
                        None
                    }
                    .unwrap_or(example.user_input);

                    if !first_line.starts_with('#') {
                        let titled_content = format!("# {}\n{}", example.title, content);
                        Some((user_input, titled_content))
                    } else {
                        Some((user_input, content))
                    }
                })
                .collect())
        } else {
            Ok(Vec::new())
        }
    };
    k().await.map_err(|e: DbErr| Error::Database(e.to_string()))
}
