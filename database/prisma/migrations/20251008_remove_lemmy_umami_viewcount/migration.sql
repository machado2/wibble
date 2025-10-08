-- Migration: remove lemmy/umami/view_count from content
BEGIN;
-- Preserve view_count into click_count (safe additive transfer)
UPDATE content
SET click_count = click_count + COALESCE(view_count, 0)
WHERE view_count IS NOT NULL AND view_count <> 0;

-- Drop indexes that reference view_count (if present)
DROP INDEX IF EXISTS idx_content_view_count;
DROP INDEX IF EXISTS idx_content_votes_view_count;

-- Drop obsolete columns
ALTER TABLE content DROP COLUMN IF EXISTS view_count;
ALTER TABLE content DROP COLUMN IF EXISTS umami_view_count;
ALTER TABLE content DROP COLUMN IF EXISTS lemmy_id;
ALTER TABLE content DROP COLUMN IF EXISTS last_lemmy_post_attempt;

COMMIT;