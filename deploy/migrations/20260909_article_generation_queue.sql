BEGIN;

ALTER TABLE content
  ADD COLUMN IF NOT EXISTS generation_status VARCHAR(20) NOT NULL DEFAULT 'pending',
  ADD COLUMN IF NOT EXISTS next_generation_at TIMESTAMP(3) NOT NULL DEFAULT CURRENT_TIMESTAMP,
  ADD COLUMN IF NOT EXISTS last_generation_error VARCHAR(500);

UPDATE content
SET generation_status = CASE
  WHEN published THEN 'completed'
  WHEN generating THEN 'pending'
  ELSE 'failed'
END
WHERE generation_status = 'pending';

ALTER TABLE content DROP CONSTRAINT IF EXISTS content_generation_status_check;
ALTER TABLE content
  ADD CONSTRAINT content_generation_status_check
  CHECK (generation_status IN ('pending', 'processing', 'retry_wait', 'completed', 'failed', 'rejected'));

CREATE INDEX IF NOT EXISTS idx_content_generation_queue
  ON content (generation_status, next_generation_at);

COMMIT;
