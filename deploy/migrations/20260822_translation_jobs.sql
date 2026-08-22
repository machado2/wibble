BEGIN;

CREATE TABLE IF NOT EXISTS translation_job (
  id CHAR(36) PRIMARY KEY,
  content_id CHAR(36) NOT NULL REFERENCES content(id) ON DELETE CASCADE,
  language_code VARCHAR(35) NOT NULL,
  requested_by VARCHAR(350) NOT NULL,
  status VARCHAR(20) NOT NULL DEFAULT 'pending',
  attempts INTEGER NOT NULL DEFAULT 0,
  created_at TIMESTAMP(0) NOT NULL DEFAULT NOW(),
  updated_at TIMESTAMP(0) NOT NULL DEFAULT NOW(),
  next_attempt_at TIMESTAMP(0) NOT NULL DEFAULT NOW(),
  started_at TIMESTAMP(0),
  completed_at TIMESTAMP(0),
  last_error VARCHAR(500),
  lease_id CHAR(36),
  CONSTRAINT translation_job_content_language_key UNIQUE (content_id, language_code),
  CONSTRAINT translation_job_status_check CHECK (status IN ('pending', 'processing', 'completed', 'failed'))
);

ALTER TABLE translation_job
  ADD COLUMN IF NOT EXISTS lease_id CHAR(36);

CREATE INDEX IF NOT EXISTS translation_job_status_next_attempt_created_idx
  ON translation_job(status, next_attempt_at, created_at);
CREATE INDEX IF NOT EXISTS translation_job_language_status_idx
  ON translation_job(language_code, status);

CREATE TABLE IF NOT EXISTS translation_worker_request (
  nonce CHAR(36) PRIMARY KEY,
  created_at TIMESTAMP(0) NOT NULL DEFAULT NOW()
);

CREATE INDEX IF NOT EXISTS translation_worker_request_created_idx
  ON translation_worker_request(created_at);

ALTER TABLE translation_job OWNER TO wibble;
ALTER TABLE translation_worker_request OWNER TO wibble;

COMMIT;
