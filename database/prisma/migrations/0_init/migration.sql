-- CreateSchema
CREATE SCHEMA IF NOT EXISTS "public";

-- CreateTable
CREATE TABLE "public"."content" (
    "id" CHAR(36) NOT NULL,
    "slug" VARCHAR(500) NOT NULL,
    "content" TEXT,
    "created_at" TIMESTAMP(6) NOT NULL DEFAULT CURRENT_TIMESTAMP,
    "generating" BOOLEAN NOT NULL DEFAULT true,
    "generation_started_at" TIMESTAMP(6),
    "generation_finished_at" TIMESTAMP(6),
    "flagged" BOOLEAN NOT NULL,
    "model" VARCHAR(100) NOT NULL,
    "prompt_version" INTEGER NOT NULL DEFAULT 1,
    "fail_count" INTEGER NOT NULL DEFAULT 0,
    "description" TEXT NOT NULL,
    "image_id" VARCHAR(100),
    "title" VARCHAR(500) NOT NULL,
    "user_input" TEXT NOT NULL,
    "view_count" INTEGER NOT NULL DEFAULT 0,
    "image_prompt" VARCHAR(1000),
    "user_email" VARCHAR(350),
    "votes" INTEGER NOT NULL DEFAULT 0,
    "hot_score" DOUBLE PRECISION NOT NULL DEFAULT 100,
    "generation_time_ms" INTEGER,
    "flarum_id" INTEGER,
    "markdown" TEXT,
    "converted" BOOLEAN NOT NULL DEFAULT false,
    "lemmy_id" INTEGER,
    "last_lemmy_post_attempt" TIMESTAMP(3),
    "longview_count" INTEGER NOT NULL DEFAULT 0,
    "umami_view_count" INTEGER NOT NULL DEFAULT 0,
    "json_content" TEXT,
    "language_id" CHAR(36),
    "click_count" INTEGER NOT NULL DEFAULT 0,
    "impression_count" INTEGER NOT NULL DEFAULT 0,

    CONSTRAINT "content_pkey" PRIMARY KEY ("id")
);

-- CreateTable
CREATE TABLE "public"."content_image" (
    "id" VARCHAR(100) NOT NULL,
    "content_id" VARCHAR(36) NOT NULL,
    "prompt_hash" VARCHAR(100),
    "prompt" TEXT NOT NULL,
    "alt_text" VARCHAR(1000) NOT NULL,
    "created_at" TIMESTAMP(6) NOT NULL,
    "flagged" BOOLEAN NOT NULL DEFAULT false,
    "regenerate" BOOLEAN NOT NULL DEFAULT false,
    "fail_count" INTEGER NOT NULL DEFAULT 0,
    "generator" VARCHAR(100),
    "model" VARCHAR(100),
    "seed" VARCHAR(20),
    "parameters" TEXT,
    "view_count" INTEGER NOT NULL DEFAULT 0,

    CONSTRAINT "content_image_pkey" PRIMARY KEY ("id")
);

-- CreateTable
CREATE TABLE "public"."content_proposal" (
    "id" CHAR(36) NOT NULL,
    "ip_address" VARCHAR(100) NOT NULL,
    "created_at" TIMESTAMP(6) NOT NULL DEFAULT CURRENT_TIMESTAMP,
    "flagged" BOOLEAN NOT NULL DEFAULT false,
    "model" VARCHAR(100) NOT NULL,
    "user_input" TEXT NOT NULL,
    "title" VARCHAR(500) NOT NULL,
    "description" TEXT NOT NULL,
    "approved_at" TIMESTAMP(6),
    "approved_by" VARCHAR(100),

    CONSTRAINT "content_proposal_pkey" PRIMARY KEY ("id")
);

-- CreateTable
CREATE TABLE "public"."content_vote" (
    "content_id" VARCHAR(36) NOT NULL,
    "created_at" TIMESTAMP(6) NOT NULL DEFAULT CURRENT_TIMESTAMP,
    "user_email" VARCHAR(350) NOT NULL,
    "downvote" BOOLEAN NOT NULL DEFAULT false,

    CONSTRAINT "content_vote_pkey" PRIMARY KEY ("content_id","user_email")
);

-- CreateTable
CREATE TABLE "public"."examples" (
    "id" CHAR(36) NOT NULL,
    "user_input" TEXT NOT NULL,
    "title" VARCHAR(500) NOT NULL,
    "description" TEXT NOT NULL,
    "content" TEXT,
    "new_id" SERIAL NOT NULL,

    CONSTRAINT "examples_pkey" PRIMARY KEY ("new_id")
);

-- CreateTable
CREATE TABLE "public"."generation_schedule" (
    "id" VARCHAR(100) NOT NULL,
    "next_run" TIMESTAMP(6) NOT NULL,

    CONSTRAINT "generation_schedule_pkey" PRIMARY KEY ("id")
);

-- CreateTable
CREATE TABLE "public"."gpt_log" (
    "id" VARCHAR(100) NOT NULL,
    "message" TEXT NOT NULL,
    "flagged" BOOLEAN NOT NULL DEFAULT false,
    "response" TEXT,
    "error" TEXT,
    "tokens" INTEGER NOT NULL,
    "created_at" TIMESTAMP(6) NOT NULL DEFAULT CURRENT_TIMESTAMP,

    CONSTRAINT "gpt_log_pkey" PRIMARY KEY ("id")
);

-- CreateTable
CREATE TABLE "public"."history_generation_fail" (
    "id" VARCHAR(100) NOT NULL,
    "slug" VARCHAR(500) NOT NULL,
    "created_at" TIMESTAMP(6) NOT NULL,
    "reason" VARCHAR(1000),
    "exception" TEXT,
    "content" TEXT,

    CONSTRAINT "history_generation_fail_pkey" PRIMARY KEY ("id")
);

-- CreateTable
CREATE TABLE "public"."horde_log" (
    "id" VARCHAR(100) NOT NULL,
    "message" TEXT NOT NULL,
    "flagged" BOOLEAN NOT NULL DEFAULT false,
    "response" TEXT,
    "error" TEXT,
    "kudos" INTEGER NOT NULL,
    "created_at" TIMESTAMP(6) NOT NULL DEFAULT CURRENT_TIMESTAMP,

    CONSTRAINT "horde_log_pkey" PRIMARY KEY ("id")
);

-- CreateTable
CREATE TABLE "public"."image_file" (
    "id" VARCHAR(100) NOT NULL,
    "file_path" TEXT NOT NULL,

    CONSTRAINT "image_file_pkey" PRIMARY KEY ("id")
);

-- CreateTable
CREATE TABLE "public"."language" (
    "id" CHAR(36) NOT NULL,
    "name" VARCHAR(500) NOT NULL,

    CONSTRAINT "language_pkey" PRIMARY KEY ("id")
);

-- CreateTable
CREATE TABLE "public"."search_history" (
    "id" VARCHAR(36) NOT NULL,
    "term" VARCHAR(1000) NOT NULL,
    "created_at" TIMESTAMP(6) NOT NULL DEFAULT CURRENT_TIMESTAMP,
    "result_count" INTEGER NOT NULL,

    CONSTRAINT "search_history_pkey" PRIMARY KEY ("id")
);

-- CreateTable
CREATE TABLE "public"."translation" (
    "id" VARCHAR(36) NOT NULL,
    "english_hash" VARCHAR(100) NOT NULL,
    "lang_id" VARCHAR(100) NOT NULL,
    "translation" TEXT NOT NULL,

    CONSTRAINT "translation_pkey" PRIMARY KEY ("id")
);

-- CreateIndex
CREATE UNIQUE INDEX "content_slug_key" ON "public"."content"("slug");

-- CreateIndex
CREATE INDEX "idx_content_created_at_generating" ON "public"."content"("created_at", "generating");

-- CreateIndex
CREATE INDEX "idx_content_hot_score" ON "public"."content"("hot_score");

-- CreateIndex
CREATE INDEX "idx_content_view_count" ON "public"."content"("view_count");

-- CreateIndex
CREATE INDEX "idx_content_votes_view_count" ON "public"."content"("votes", "view_count");

-- CreateIndex
CREATE INDEX "content_image_content_id_prompt_hash_idx" ON "public"."content_image"("content_id", "prompt_hash");

-- CreateIndex
CREATE INDEX "content_image_created_at_idx" ON "public"."content_image"("created_at");

-- CreateIndex
CREATE INDEX "content_image_view_count_idx" ON "public"."content_image"("view_count");

-- CreateIndex
CREATE UNIQUE INDEX "content_image_content_id_prompt_hash_key" ON "public"."content_image"("content_id", "prompt_hash");

-- CreateIndex
CREATE INDEX "idx_uuid" ON "public"."examples"("id");

-- CreateIndex
CREATE INDEX "gpt_log_created_at_idx" ON "public"."gpt_log"("created_at");

-- CreateIndex
CREATE INDEX "history_generation_fail_created_at_idx" ON "public"."history_generation_fail"("created_at");

-- CreateIndex
CREATE INDEX "horde_log_created_at_idx" ON "public"."horde_log"("created_at");

-- CreateIndex
CREATE UNIQUE INDEX "language_name_key" ON "public"."language"("name");

-- CreateIndex
CREATE INDEX "idx_search_result_count_created_at" ON "public"."search_history"("result_count", "created_at");

-- AddForeignKey
ALTER TABLE "public"."content_image" ADD CONSTRAINT "content_image_content_id_fkey" FOREIGN KEY ("content_id") REFERENCES "public"."content"("id") ON DELETE NO ACTION ON UPDATE NO ACTION;

-- AddForeignKey
ALTER TABLE "public"."content_vote" ADD CONSTRAINT "content_vote_content_id_fkey" FOREIGN KEY ("content_id") REFERENCES "public"."content"("id") ON DELETE CASCADE ON UPDATE NO ACTION;

-- AddForeignKey
ALTER TABLE "public"."translation" ADD CONSTRAINT "translation_lang_id_fkey" FOREIGN KEY ("lang_id") REFERENCES "public"."language"("id") ON DELETE RESTRICT ON UPDATE NO ACTION;

