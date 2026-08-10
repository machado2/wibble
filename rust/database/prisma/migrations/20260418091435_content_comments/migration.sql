CREATE TABLE "public"."content_comment" (
    "id" VARCHAR(36) NOT NULL,
    "content_id" VARCHAR(36) NOT NULL,
    "user_email" VARCHAR(350) NOT NULL,
    "user_name" VARCHAR(500) NOT NULL,
    "body" TEXT NOT NULL,
    "created_at" TIMESTAMP(6) NOT NULL DEFAULT CURRENT_TIMESTAMP,

    CONSTRAINT "content_comment_pkey" PRIMARY KEY ("id")
);

CREATE INDEX "idx_content_comment_content_created_at"
ON "public"."content_comment"("content_id", "created_at");

CREATE INDEX "idx_content_comment_user_created_at"
ON "public"."content_comment"("user_email", "created_at");

ALTER TABLE "public"."content_comment"
ADD CONSTRAINT "content_comment_content_id_fkey"
FOREIGN KEY ("content_id") REFERENCES "public"."content"("id")
ON DELETE CASCADE ON UPDATE NO ACTION;
