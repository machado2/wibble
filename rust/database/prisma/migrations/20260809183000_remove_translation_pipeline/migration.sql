DROP TABLE IF EXISTS "public"."translation_job";
DROP TABLE IF EXISTS "public"."translation";
DROP TABLE IF EXISTS "public"."language";
ALTER TABLE "public"."content" DROP COLUMN IF EXISTS "language_id";
