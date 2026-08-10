ALTER TABLE "public"."content"
ADD COLUMN IF NOT EXISTS "recovered_from_dead_link" BOOLEAN NOT NULL DEFAULT false;
