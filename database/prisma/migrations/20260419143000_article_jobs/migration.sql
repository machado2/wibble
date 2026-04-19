CREATE TABLE "public"."article_job" (
    "id" VARCHAR(36) NOT NULL,
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
    "finished_at" TIMESTAMP(6),

    CONSTRAINT "article_job_pkey" PRIMARY KEY ("id")
);

CREATE INDEX "idx_article_job_article_id"
ON "public"."article_job"("article_id");

CREATE INDEX "idx_article_job_status_phase_created_at"
ON "public"."article_job"("status", "phase", "created_at");

CREATE INDEX "idx_article_job_requester_key_created_at"
ON "public"."article_job"("requester_key", "created_at");
