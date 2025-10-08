use dotenvy::dotenv;
use sea_orm::{ConnectionTrait, Database, DbErr, Statement, DbConn, DbBackend};
use std::env;
use tracing::{info, error};

#[tokio::main]
async fn main() -> Result<(), ()> {
    // Small CLI utility to recompute and persist `hot_score` in the DB.
    // Run periodically (cron/systemd timer) to keep the indexed hot_score up-to-date
    // so the application can order by it in SQL.
    //
    // NOTE: This does not attempt to drop/alter columns (migrations should be done
    // separately). Example SQL to run in migration to remove `view_count`, `umami_view_count`
    // and lemmy fields (run only after you've migrated values you need):
    //
    // ALTER TABLE content DROP COLUMN IF EXISTS view_count;
    // ALTER TABLE content DROP COLUMN IF EXISTS umami_view_count;
    // ALTER TABLE content DROP COLUMN IF EXISTS lemmy_id;
    // ALTER TABLE content DROP COLUMN IF EXISTS last_lemmy_post_attempt;
    //
    // The score formula used here mirrors the previous Rust in-memory heuristic:
    // hot = click_rate * 0.7 + (1 / age_hours) * 0.3
    // where click_rate = click_count / impression_count (0 when impressions = 0)
    //
    // We compute age_hours using PostgreSQL EXTRACT(EPOCH FROM (now() - created_at))/3600
    // and use GREATEST(..., 1) to avoid division by zero and match previous .max(1).

    dotenv().ok();
    tracing_subscriber::fmt::init();

    let db_url = match env::var("DATABASE_URL") {
        Ok(v) => v,
        Err(_) => {
            error!("DATABASE_URL environment variable is not set");
            return Err(());
        }
    };

    let db = match Database::connect(&db_url).await {
        Ok(db) => db,
        Err(e) => {
            error!("Failed to connect to database: {}", e);
            return Err(());
        }
    };

    if let Err(e) = update_hot_scores(&db).await {
        error!("Error updating hot scores: {}", e);
        return Err(());
    }

    info!("Hot score update finished successfully");
    Ok(())
}

async fn update_hot_scores(db: &DbConn) -> Result<(), DbErr> {
    // PostgreSQL expression implementing the hot score formula
    let sql = r#"
    UPDATE content
    SET hot_score = (
        (CASE WHEN impression_count > 0
            THEN (click_count::double precision / impression_count::double precision)
            ELSE 0.0
        END) * 0.7
    )
    + (
        (1.0 / GREATEST(EXTRACT(EPOCH FROM (now() - created_at)) / 3600.0, 1.0))
        * 0.3
    )
    WHERE generating = false AND flagged = false;
    "#;

    // Use Postgres backend here because schema is Postgres; adjust if using another DB.
    let stmt = Statement::from_string(DbBackend::Postgres, sql.to_string());
    db.execute(stmt).await.map(|_| ())
}