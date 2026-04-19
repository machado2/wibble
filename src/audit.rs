use sea_orm::{DatabaseConnection, EntityTrait};
use uuid::Uuid;

use crate::auth::AuthUser;
use crate::entities::{audit_log, prelude::*};
use crate::error::Error;

pub async fn log_audit(
    db: &DatabaseConnection,
    user: &AuthUser,
    action: &str,
    target_type: &str,
    target_id: &str,
    details: Option<String>,
) -> Result<(), Error> {
    let log = audit_log::Model {
        id: Uuid::new_v4().to_string(),
        user_email: user.email.clone(),
        user_name: Some(user.name.clone()),
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
