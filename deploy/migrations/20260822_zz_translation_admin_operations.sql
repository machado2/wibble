BEGIN;

CREATE TABLE IF NOT EXISTS translation_runtime_setting (
  id VARCHAR(40) PRIMARY KEY,
  hourly_limit INTEGER NOT NULL DEFAULT 10,
  budget_reset_at TIMESTAMP(3),
  updated_at TIMESTAMP(3) NOT NULL DEFAULT CURRENT_TIMESTAMP,
  CONSTRAINT translation_runtime_setting_hourly_limit_check
    CHECK (hourly_limit >= 0 AND hourly_limit <= 100),
  CONSTRAINT translation_runtime_setting_id_check
    CHECK (id = 'automatic')
);

ALTER TABLE translation_runtime_setting OWNER TO wibble;

ALTER TABLE translation_runtime_setting
  ADD COLUMN IF NOT EXISTS hourly_limit INTEGER NOT NULL DEFAULT 10,
  ADD COLUMN IF NOT EXISTS budget_reset_at TIMESTAMP(3),
  ADD COLUMN IF NOT EXISTS updated_at TIMESTAMP(3) NOT NULL DEFAULT CURRENT_TIMESTAMP;

ALTER TABLE translation_runtime_setting
  ALTER COLUMN budget_reset_at TYPE TIMESTAMP(3),
  ALTER COLUMN updated_at TYPE TIMESTAMP(3);

ALTER TABLE translation_generation_attempt
  ALTER COLUMN created_at TYPE TIMESTAMP(3);

INSERT INTO translation_runtime_setting (id, hourly_limit)
VALUES ('automatic', 10)
ON CONFLICT (id) DO NOTHING;

CREATE TABLE IF NOT EXISTS translation_admin_event (
  id CHAR(36) PRIMARY KEY,
  action VARCHAR(50) NOT NULL,
  job_id CHAR(36),
  numeric_value INTEGER,
  created_at TIMESTAMP(3) NOT NULL DEFAULT CURRENT_TIMESTAMP
);

ALTER TABLE translation_admin_event OWNER TO wibble;

ALTER TABLE translation_admin_event
  ADD COLUMN IF NOT EXISTS action VARCHAR(50),
  ADD COLUMN IF NOT EXISTS job_id CHAR(36),
  ADD COLUMN IF NOT EXISTS numeric_value INTEGER,
  ADD COLUMN IF NOT EXISTS created_at TIMESTAMP(3) NOT NULL DEFAULT CURRENT_TIMESTAMP;

ALTER TABLE translation_admin_event
  ALTER COLUMN action SET NOT NULL,
  ALTER COLUMN created_at TYPE TIMESTAMP(3);

CREATE INDEX IF NOT EXISTS translation_admin_event_created_at_idx
  ON translation_admin_event (created_at);

-- Legacy automatic work was admitted before quota-aware queue admission existed.
-- A fixed audit marker makes this cleanup one-shot even if the SQL is reapplied.
WITH cleanup_marker AS (
  INSERT INTO translation_admin_event (id, action, job_id, numeric_value)
  VALUES ('00000000-0000-4000-8000-000000000001', 'legacy_queue_rejected', NULL, NULL)
  ON CONFLICT (id) DO NOTHING
  RETURNING id
)
UPDATE translation_job
SET status = 'failed',
    completed_at = CURRENT_TIMESTAMP,
    updated_at = CURRENT_TIMESTAMP,
    next_attempt_at = CURRENT_TIMESTAMP,
    last_error = 'Quota de traduções esgotada',
    lease_id = NULL
WHERE status = 'pending'
  AND requested_by = 'background-translations@wibble.internal'
  AND EXISTS (SELECT 1 FROM cleanup_marker);

COMMIT;
