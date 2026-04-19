use sea_orm::{
    ConnectionTrait, DatabaseConnection, DbBackend, EntityTrait, QuerySelect, Statement,
};

use crate::entities::prelude::*;
use crate::error::Error;

// Startup ALTER TABLEs remain a temporary compatibility bridge until proper migrations
// fully replace them before release.
const STARTUP_SCHEMA_COMPATIBILITY_MODE: &str = "temporary-startup-bridge";

const ASYNC_IMAGE_JOB_COMPATIBILITY: &[&str] = &[
    r#"ALTER TABLE "public"."content_image"
       ADD COLUMN IF NOT EXISTS "status" VARCHAR(32) NOT NULL DEFAULT 'completed'"#,
    r#"ALTER TABLE "public"."content_image"
       ADD COLUMN IF NOT EXISTS "last_error" TEXT"#,
    r#"ALTER TABLE "public"."content_image"
       ADD COLUMN IF NOT EXISTS "generation_started_at" TIMESTAMP(6)"#,
    r#"ALTER TABLE "public"."content_image"
       ADD COLUMN IF NOT EXISTS "generation_finished_at" TIMESTAMP(6)"#,
    r#"ALTER TABLE "public"."content_image"
       ADD COLUMN IF NOT EXISTS "provider_job_id" VARCHAR(100)"#,
    r#"ALTER TABLE "public"."content_image"
       ADD COLUMN IF NOT EXISTS "provider_job_url" VARCHAR(1000)"#,
    r#"UPDATE "public"."content_image"
       SET "status" = 'completed'
       WHERE "status" IS NULL"#,
    r#"CREATE INDEX IF NOT EXISTS "content_image_status_created_at_idx"
       ON "public"."content_image"("status", "created_at")"#,
];

const AUTH_COMPATIBILITY: &[&str] = &[
    r#"ALTER TABLE "public"."content"
       ADD COLUMN IF NOT EXISTS "author_email" VARCHAR(350)"#,
    r#"CREATE TABLE IF NOT EXISTS "public"."audit_log" (
        "id" VARCHAR(36) PRIMARY KEY,
        "user_email" VARCHAR(350) NOT NULL,
        "user_name" VARCHAR(500),
        "action" VARCHAR(100) NOT NULL,
        "target_type" VARCHAR(50) NOT NULL,
        "target_id" VARCHAR(500) NOT NULL,
        "details" TEXT,
        "created_at" TIMESTAMP(6) DEFAULT NOW()
    )"#,
    r#"ALTER TABLE "public"."audit_log"
       ALTER COLUMN "target_id" TYPE VARCHAR(500)"#,
    r#"CREATE INDEX IF NOT EXISTS "audit_log_created_at_idx"
       ON "public"."audit_log"("created_at")"#,
    r#"CREATE INDEX IF NOT EXISTS "audit_log_target_idx"
       ON "public"."audit_log"("target_type", "target_id")"#,
    r#"ALTER TABLE "public"."content"
       ADD COLUMN IF NOT EXISTS "published" BOOLEAN NOT NULL DEFAULT true"#,
    r#"ALTER TABLE "public"."content"
       ADD COLUMN IF NOT EXISTS "recovered_from_dead_link" BOOLEAN NOT NULL DEFAULT false"#,
];

const COMMENT_TABLE_COMPATIBILITY: &[&str] = &[
    r#"CREATE TABLE IF NOT EXISTS "public"."content_comment" (
        "id" VARCHAR(36) PRIMARY KEY,
        "content_id" VARCHAR(36) NOT NULL,
        "user_email" VARCHAR(350) NOT NULL,
        "user_name" VARCHAR(500) NOT NULL,
        "body" TEXT NOT NULL,
        "created_at" TIMESTAMP(6) DEFAULT NOW()
    )"#,
    r#"CREATE INDEX IF NOT EXISTS "idx_content_comment_content_created_at"
       ON "public"."content_comment"("content_id", "created_at")"#,
    r#"CREATE INDEX IF NOT EXISTS "idx_content_comment_user_created_at"
       ON "public"."content_comment"("user_email", "created_at")"#,
    r#"DO $$
    BEGIN
        IF NOT EXISTS (
            SELECT 1
            FROM pg_constraint
            WHERE conname = 'content_comment_content_id_fkey'
        ) THEN
            ALTER TABLE "public"."content_comment"
            ADD CONSTRAINT "content_comment_content_id_fkey"
            FOREIGN KEY ("content_id") REFERENCES "public"."content"("id")
            ON DELETE CASCADE ON UPDATE NO ACTION;
        END IF;
    END $$"#,
];

const TRANSLATION_JOB_COMPATIBILITY: &[&str] = &[
    r#"CREATE TABLE IF NOT EXISTS "public"."translation_job" (
        "id" VARCHAR(100) PRIMARY KEY,
        "article_id" VARCHAR(36) NOT NULL REFERENCES "public"."content"("id") ON DELETE CASCADE ON UPDATE NO ACTION,
        "language_code" VARCHAR(16) NOT NULL,
        "request_source" VARCHAR(32) NOT NULL,
        "priority" INTEGER NOT NULL DEFAULT 0,
        "status" VARCHAR(32) NOT NULL DEFAULT 'queued',
        "fail_count" INTEGER NOT NULL DEFAULT 0,
        "last_error" TEXT,
        "created_at" TIMESTAMP(6) NOT NULL DEFAULT NOW(),
        "updated_at" TIMESTAMP(6) NOT NULL DEFAULT NOW(),
        "started_at" TIMESTAMP(6),
        "finished_at" TIMESTAMP(6),
        "next_retry_at" TIMESTAMP(6)
    )"#,
    r#"CREATE INDEX IF NOT EXISTS "idx_translation_job_article_id"
       ON "public"."translation_job"("article_id")"#,
    r#"CREATE INDEX IF NOT EXISTS "idx_translation_job_status_priority_created_at"
       ON "public"."translation_job"("status", "priority", "created_at")"#,
];

const ARTICLE_JOB_COMPATIBILITY: &[&str] = &[
    r#"CREATE TABLE IF NOT EXISTS "public"."article_job" (
        "id" VARCHAR(36) PRIMARY KEY,
        "article_id" VARCHAR(36),
        "requester_key" VARCHAR(500) NOT NULL,
        "requester_tier" VARCHAR(32) NOT NULL,
        "author_email" VARCHAR(350),
        "prompt" TEXT NOT NULL,
        "feature_type" VARCHAR(64) NOT NULL,
        "phase" VARCHAR(32) NOT NULL DEFAULT 'queued',
        "status" VARCHAR(32) NOT NULL DEFAULT 'queued',
        "usage_counters" TEXT,
        "preview_payload" TEXT,
        "error_summary" TEXT,
        "fail_count" INTEGER NOT NULL DEFAULT 0,
        "created_at" TIMESTAMP(6) NOT NULL DEFAULT NOW(),
        "updated_at" TIMESTAMP(6) NOT NULL DEFAULT NOW(),
        "started_at" TIMESTAMP(6),
        "finished_at" TIMESTAMP(6)
    )"#,
    r#"CREATE INDEX IF NOT EXISTS "idx_article_job_article_id"
       ON "public"."article_job"("article_id")"#,
    r#"CREATE INDEX IF NOT EXISTS "idx_article_job_status_phase_created_at"
       ON "public"."article_job"("status", "phase", "created_at")"#,
    r#"CREATE INDEX IF NOT EXISTS "idx_article_job_requester_key_created_at"
       ON "public"."article_job"("requester_key", "created_at")"#,
];

pub fn startup_schema_compatibility_mode() -> &'static str {
    STARTUP_SCHEMA_COMPATIBILITY_MODE
}

pub async fn apply_startup_schema_compatibility(db: &DatabaseConnection) {
    apply_compatibility_statements(
        db,
        ASYNC_IMAGE_JOB_COMPATIBILITY,
        "async image job schema compatibility",
    )
    .await;
    apply_compatibility_statements(db, AUTH_COMPATIBILITY, "auth schema compatibility").await;
    apply_compatibility_statements(
        db,
        COMMENT_TABLE_COMPATIBILITY,
        "comment schema compatibility",
    )
    .await;
    apply_compatibility_statements(
        db,
        TRANSLATION_JOB_COMPATIBILITY,
        "translation job schema compatibility",
    )
    .await;
    apply_compatibility_statements(
        db,
        ARTICLE_JOB_COMPATIBILITY,
        "article job schema compatibility",
    )
    .await;
}

pub async fn validate_required_schema(db: &DatabaseConnection) -> Result<(), Error> {
    Content::find()
        .limit(1)
        .all(db)
        .await
        .map_err(|e| Error::Database(format!("Content schema validation failed: {}", e)))?;
    ContentImage::find()
        .limit(1)
        .all(db)
        .await
        .map_err(|e| Error::Database(format!("ContentImage schema validation failed: {}", e)))?;
    ContentComment::find()
        .limit(1)
        .all(db)
        .await
        .map_err(|e| Error::Database(format!("ContentComment schema validation failed: {}", e)))?;
    AuditLog::find()
        .limit(1)
        .all(db)
        .await
        .map_err(|e| Error::Database(format!("AuditLog schema validation failed: {}", e)))?;
    TranslationJob::find()
        .limit(1)
        .all(db)
        .await
        .map_err(|e| Error::Database(format!("TranslationJob schema validation failed: {}", e)))?;
    ArticleJob::find()
        .limit(1)
        .all(db)
        .await
        .map_err(|e| Error::Database(format!("ArticleJob schema validation failed: {}", e)))?;
    Ok(())
}

async fn apply_compatibility_statements(
    db: &DatabaseConnection,
    statements: &[&str],
    context: &str,
) {
    for sql in statements {
        let stmt = Statement::from_string(DbBackend::Postgres, (*sql).to_string());
        if let Err(err) = db.execute(stmt).await {
            eprintln!("Error ensuring {}: {}", context, err);
        }
    }
}
