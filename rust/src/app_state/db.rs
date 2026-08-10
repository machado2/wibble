use std::env;

use sea_orm::{Database, DatabaseConnection};

use crate::error::Error;

pub async fn connect_database() -> Result<DatabaseConnection, Error> {
    let connection = env::var("DATABASE_URL")
        .map_err(|_| Error::Database("DATABASE_URL must be set".to_string()))?;
    Database::connect(connection)
        .await
        .map_err(|e| Error::Database(format!("Failed to connect to database: {}", e)))
}
