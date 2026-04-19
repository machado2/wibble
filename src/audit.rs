use sea_orm::{DatabaseConnection, EntityTrait};
use uuid::Uuid;

use crate::auth::AuthUser;
use crate::entities::{audit_log, prelude::*};
use crate::error::Error;

const SYSTEM_AUDIT_EMAIL: &str = "system@wibble.local";
const SYSTEM_AUDIT_NAME: &str = "Wibble System";

async fn insert_audit_log(
    db: &DatabaseConnection,
    user_email: String,
    user_name: Option<String>,
    action: &str,
    target_type: &str,
    target_id: &str,
    details: Option<String>,
) -> Result<(), Error> {
    let log = audit_log::Model {
        id: Uuid::new_v4().to_string(),
        user_email,
        user_name,
        action: action.to_string(),
        target_type: target_type.to_string(),
        target_id: target_id.to_string(),
        details,
        created_at: chrono::Utc::now().naive_local(),
    };

    AuditLog::insert(audit_log::ActiveModel::from(log))
        .exec(db)
        .await
        .map_err(|e| Error::Database(format!("Error inserting audit log: {}", e)))?;

    Ok(())
}

pub async fn log_audit(
    db: &DatabaseConnection,
    user: &AuthUser,
    action: &str,
    target_type: &str,
    target_id: &str,
    details: Option<String>,
) -> Result<(), Error> {
    insert_audit_log(
        db,
        user.email.clone(),
        Some(user.name.clone()),
        action,
        target_type,
        target_id,
        details,
    )
    .await
}

pub async fn log_system_audit(
    db: &DatabaseConnection,
    action: &str,
    target_type: &str,
    target_id: &str,
    details: Option<String>,
) -> Result<(), Error> {
    insert_audit_log(
        db,
        SYSTEM_AUDIT_EMAIL.to_string(),
        Some(SYSTEM_AUDIT_NAME.to_string()),
        action,
        target_type,
        target_id,
        details,
    )
    .await
}
