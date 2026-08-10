-- AlterTable
ALTER TABLE "public"."content_image"
ADD COLUMN "status" VARCHAR(32) NOT NULL DEFAULT 'completed',
ADD COLUMN "last_error" TEXT,
ADD COLUMN "generation_started_at" TIMESTAMP(6),
ADD COLUMN "generation_finished_at" TIMESTAMP(6),
ADD COLUMN "provider_job_id" VARCHAR(100),
ADD COLUMN "provider_job_url" VARCHAR(1000);

-- CreateIndex
CREATE INDEX "content_image_status_created_at_idx"
ON "public"."content_image"("status", "created_at");
