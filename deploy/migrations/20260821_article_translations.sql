CREATE TABLE IF NOT EXISTS content_translation (
  id CHAR(36) PRIMARY KEY,
  content_id CHAR(36) NOT NULL,
  language_code VARCHAR(35) NOT NULL,
  title VARCHAR(500) NOT NULL,
  description TEXT NOT NULL,
  content TEXT NOT NULL,
  model VARCHAR(100) NOT NULL,
  created_at TIMESTAMP(0) NOT NULL DEFAULT CURRENT_TIMESTAMP
);

CREATE INDEX IF NOT EXISTS content_translation_content_id_idx
  ON content_translation (content_id);

CREATE UNIQUE INDEX IF NOT EXISTS content_translation_content_id_language_code_key
  ON content_translation (content_id, language_code);

DO $$
BEGIN
  IF NOT EXISTS (
    SELECT 1 FROM pg_constraint
    WHERE conname = 'content_translation_content_id_fkey'
      AND conrelid = 'content_translation'::regclass
      AND contype = 'f'
  ) THEN
    ALTER TABLE content_translation
      ADD CONSTRAINT content_translation_content_id_fkey
      FOREIGN KEY (content_id) REFERENCES content(id)
      ON DELETE CASCADE ON UPDATE CASCADE;
  END IF;
END
$$;

ALTER TABLE content_translation OWNER TO wibble;

CREATE TABLE IF NOT EXISTS translation_generation_attempt (
  id CHAR(36) PRIMARY KEY,
  user_email VARCHAR(350) NOT NULL,
  content_id CHAR(36) NOT NULL,
  language_code VARCHAR(35) NOT NULL,
  created_at TIMESTAMP(0) NOT NULL DEFAULT CURRENT_TIMESTAMP
);

CREATE INDEX IF NOT EXISTS translation_generation_attempt_user_created_idx
  ON translation_generation_attempt (user_email, created_at);

CREATE INDEX IF NOT EXISTS translation_generation_attempt_content_language_idx
  ON translation_generation_attempt (content_id, language_code);

DO $$
BEGIN
  IF NOT EXISTS (
    SELECT 1 FROM pg_constraint
    WHERE conname = 'translation_generation_attempt_content_id_fkey'
      AND conrelid = 'translation_generation_attempt'::regclass
      AND contype = 'f'
  ) THEN
    ALTER TABLE translation_generation_attempt
      ADD CONSTRAINT translation_generation_attempt_content_id_fkey
      FOREIGN KEY (content_id) REFERENCES content(id)
      ON DELETE CASCADE ON UPDATE CASCADE;
  END IF;
END
$$;

ALTER TABLE translation_generation_attempt OWNER TO wibble;
