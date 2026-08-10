CREATE TABLE IF NOT EXISTS not_found_request (
  id VARCHAR(36) PRIMARY KEY,
  url VARCHAR(2000) NOT NULL UNIQUE,
  hit_count INTEGER NOT NULL DEFAULT 1,
  first_seen_at TIMESTAMP(0) NOT NULL DEFAULT CURRENT_TIMESTAMP,
  last_seen_at TIMESTAMP(0) NOT NULL DEFAULT CURRENT_TIMESTAMP,
  generated_slug VARCHAR(500),
  generated_at TIMESTAMP(0)
);

CREATE INDEX IF NOT EXISTS idx_not_found_request_admin
  ON not_found_request (generated_at, hit_count, last_seen_at);
