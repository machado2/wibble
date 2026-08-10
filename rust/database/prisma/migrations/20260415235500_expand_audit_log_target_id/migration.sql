-- The audit_log model predates the migration history shipped in some Wibble
-- checkouts. Keep fresh installs and upgraded installs equivalent.
CREATE TABLE IF NOT EXISTS "public"."audit_log" (
    "id" VARCHAR(36) NOT NULL,
    "user_email" VARCHAR(350) NOT NULL,
    "user_name" VARCHAR(500),
    "action" VARCHAR(100) NOT NULL,
    "target_type" VARCHAR(50) NOT NULL,
    "target_id" VARCHAR(500) NOT NULL,
    "details" TEXT,
    "created_at" TIMESTAMP(6) NOT NULL DEFAULT CURRENT_TIMESTAMP,

    CONSTRAINT "audit_log_pkey" PRIMARY KEY ("id")
);

-- AlterTable
ALTER TABLE "public"."audit_log"
ALTER COLUMN "target_id" TYPE VARCHAR(500);

-- CreateIndex
CREATE INDEX IF NOT EXISTS "idx_audit_log_created_at" ON "public"."audit_log"("created_at");
CREATE INDEX IF NOT EXISTS "idx_audit_log_target" ON "public"."audit_log"("target_type", "target_id");
