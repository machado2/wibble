CREATE TABLE "public"."translation_job" (
    "id" VARCHAR(100) NOT NULL,
    "article_id" VARCHAR(36) NOT NULL,
    "language_code" VARCHAR(16) NOT NULL,
    "request_source" VARCHAR(32) NOT NULL,
    "priority" INTEGER NOT NULL DEFAULT 0,
    "status" VARCHAR(32) NOT NULL DEFAULT 'queued',
    "fail_count" INTEGER NOT NULL DEFAULT 0,
    "last_error" TEXT,
    "created_at" TIMESTAMP(6) NOT NULL DEFAULT CURRENT_TIMESTAMP,
    "updated_at" TIMESTAMP(6) NOT NULL DEFAULT CURRENT_TIMESTAMP,
    "started_at" TIMESTAMP(6),
    "finished_at" TIMESTAMP(6),
    "next_retry_at" TIMESTAMP(6),

    CONSTRAINT "translation_job_pkey" PRIMARY KEY ("id")
);

CREATE INDEX "idx_translation_job_article_id"
ON "public"."translation_job"("article_id");

CREATE INDEX "idx_translation_job_status_priority_created_at"
ON "public"."translation_job"("status", "priority", "created_at");

ALTER TABLE "public"."translation_job"
ADD CONSTRAINT "translation_job_article_id_fkey"
FOREIGN KEY ("article_id") REFERENCES "public"."content"("id")
ON DELETE CASCADE ON UPDATE NO ACTION;
