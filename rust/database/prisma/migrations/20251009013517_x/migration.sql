/*
  Warnings:

  - You are about to drop the column `last_lemmy_post_attempt` on the `content` table. All the data in the column will be lost.
  - You are about to drop the column `lemmy_id` on the `content` table. All the data in the column will be lost.
  - You are about to drop the column `umami_view_count` on the `content` table. All the data in the column will be lost.
  - You are about to drop the column `view_count` on the `content` table. All the data in the column will be lost.

*/
-- DropIndex
DROP INDEX "public"."idx_content_view_count";

-- DropIndex
DROP INDEX "public"."idx_content_votes_view_count";

-- AlterTable
ALTER TABLE "public"."content" DROP COLUMN "last_lemmy_post_attempt",
DROP COLUMN "lemmy_id",
DROP COLUMN "umami_view_count",
DROP COLUMN "view_count";
