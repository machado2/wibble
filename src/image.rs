use sea_orm::prelude::*;

use crate::entities::prelude::*;
use crate::error::{Error, Result};

pub async fn get_image(db: &DatabaseConnection, id: &str) -> Result<Vec<u8>> {
    let i = ImageData::find_by_id(id)
        .one(db)
        .await
        .map_err(|e| Error::Database(format!("Database error reading image: {}", e)))?
        .ok_or(Error::NotFound)?;
    Ok(i.jpeg_data)
}
