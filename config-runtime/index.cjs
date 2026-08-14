const { execFileSync } = require("node:child_process");
const fs = require("node:fs");
const path = require("node:path");

let cache = null;
let cachedPath = null;
let cachedMtimeMs = -1;
let failedMtimeMs = -1;

const requiredString = (value, field) => {
  if (typeof value !== "string" || value.trim().length === 0) {
    throw new Error(`Invalid Nickel configuration: ${field} must be a string`);
  }
  return value.trim();
};

const requiredBoolean = (value, field) => {
  if (typeof value !== "boolean") {
    throw new Error(`Invalid Nickel configuration: ${field} must be a boolean`);
  }
  return value;
};

const requiredNumber = (value, field, minimum = 0) => {
  if (typeof value !== "number" || !Number.isFinite(value) || value < minimum) {
    throw new Error(
      `Invalid Nickel configuration: ${field} must be at least ${minimum}`
    );
  }
  return value;
};

const requiredStringArray = (value, field) => {
  if (!Array.isArray(value)) {
    throw new Error(`Invalid Nickel configuration: ${field} must be an array`);
  }
  return value.map((entry, index) =>
    requiredString(entry, `${field}[${index}]`)
  );
};

const secretString = (value, field) => {
  if (typeof value !== "string") {
    throw new Error(`Invalid Nickel configuration: ${field} must be a string`);
  }
  return value;
};

const validateWriter = (writer, index) => {
  const field = `generation.writers[${index}]`;
  const provider = requiredString(writer?.provider, `${field}.provider`);
  if (provider !== "openai" && provider !== "openrouter") {
    throw new Error(
      `Invalid Nickel configuration: ${field}.provider must be openai or openrouter`
    );
  }
  return {
    id: requiredString(writer?.id, `${field}.id`),
    nickname: requiredString(writer?.nickname, `${field}.nickname`),
    slug: requiredString(writer?.slug, `${field}.slug`),
    provider,
    available: requiredBoolean(writer?.available, `${field}.available`),
    admin_only: requiredBoolean(writer?.admin_only, `${field}.admin_only`),
  };
};

const validateConfig = (raw) => {
  if (!raw || typeof raw !== "object") {
    throw new Error("Invalid Nickel configuration: expected a record");
  }
  const writers = Array.isArray(raw.generation?.writers)
    ? raw.generation.writers.map(validateWriter)
    : null;
  if (!writers || writers.length === 0) {
    throw new Error(
      "Invalid Nickel configuration: generation.writers cannot be empty"
    );
  }
  const ids = new Set();
  for (const writer of writers) {
    if (ids.has(writer.id)) {
      throw new Error(
        `Invalid Nickel configuration: duplicate writer id ${writer.id}`
      );
    }
    ids.add(writer.id);
  }

  return {
    secrets: {
      database_url: secretString(
        raw.secrets?.database_url,
        "secrets.database_url"
      ),
      nextauth_secret: secretString(
        raw.secrets?.nextauth_secret,
        "secrets.nextauth_secret"
      ),
      sso_client_secret: secretString(
        raw.secrets?.sso_client_secret,
        "secrets.sso_client_secret"
      ),
      admin_email: secretString(
        raw.secrets?.admin_email,
        "secrets.admin_email"
      ),
      safety_identifier_secret: secretString(
        raw.secrets?.safety_identifier_secret,
        "secrets.safety_identifier_secret"
      ),
      openai_api_key: secretString(
        raw.secrets?.openai_api_key,
        "secrets.openai_api_key"
      ),
      openrouter_api_key: secretString(
        raw.secrets?.openrouter_api_key,
        "secrets.openrouter_api_key"
      ),
      replicate_api_token: secretString(
        raw.secrets?.replicate_api_token,
        "secrets.replicate_api_token"
      ),
      loggly_token: secretString(
        raw.secrets?.loggly_token,
        "secrets.loggly_token"
      ),
    },
    app: {
      site_url: requiredString(raw.app?.site_url, "app.site_url"),
      sso_public_url: requiredString(
        raw.app?.sso_public_url,
        "app.sso_public_url"
      ),
      discord_url: requiredString(raw.app?.discord_url, "app.discord_url"),
      images_dir: requiredString(raw.app?.images_dir, "app.images_dir"),
    },
    auth: {
      sso_issuer_url: requiredString(
        raw.auth?.sso_issuer_url,
        "auth.sso_issuer_url"
      ),
      sso_client_id: requiredString(
        raw.auth?.sso_client_id,
        "auth.sso_client_id"
      ),
    },
    generation: {
      writers,
      openai_api_url: requiredString(
        raw.generation?.openai_api_url,
        "generation.openai_api_url"
      ),
      openrouter_api_url: requiredString(
        raw.generation?.openrouter_api_url,
        "generation.openrouter_api_url"
      ),
      moderation_enabled: requiredBoolean(
        raw.generation?.moderation_enabled,
        "generation.moderation_enabled"
      ),
      moderation_api_url: requiredString(
        raw.generation?.moderation_api_url,
        "generation.moderation_api_url"
      ),
      max_output_tokens: requiredNumber(
        raw.generation?.max_output_tokens,
        "generation.max_output_tokens",
        1
      ),
      max_prompt_length: requiredNumber(
        raw.generation?.max_prompt_length,
        "generation.max_prompt_length",
        1
      ),
    },
    image: {
      mode: requiredString(raw.image?.mode, "image.mode"),
      replicate_api_url: requiredString(
        raw.image?.replicate_api_url,
        "image.replicate_api_url"
      ),
      minimum_request_interval_seconds: requiredNumber(
        raw.image?.minimum_request_interval_seconds,
        "image.minimum_request_interval_seconds",
        1
      ),
    },
    worker: {
      cool_down_seconds: requiredNumber(
        raw.worker?.cool_down_seconds,
        "worker.cool_down_seconds"
      ),
      idle_sleep_seconds: requiredNumber(
        raw.worker?.idle_sleep_seconds,
        "worker.idle_sleep_seconds"
      ),
      wait_on_error_seconds: requiredNumber(
        raw.worker?.wait_on_error_seconds,
        "worker.wait_on_error_seconds"
      ),
      update_scores_interval_seconds: requiredNumber(
        raw.worker?.update_scores_interval_seconds,
        "worker.update_scores_interval_seconds",
        1
      ),
    },
    logging: {
      loggly_subdomain: requiredString(
        raw.logging?.loggly_subdomain,
        "logging.loggly_subdomain"
      ),
      web_tags: requiredStringArray(raw.logging?.web_tags, "logging.web_tags"),
      worker_tags: requiredStringArray(
        raw.logging?.worker_tags,
        "logging.worker_tags"
      ),
    },
  };
};

const findConfigPath = () => {
  const configuredPath = process.env.WIBBLE_CONFIG_PATH?.trim();
  if (configuredPath) {
    return path.resolve(configuredPath);
  }

  let directory = process.cwd();
  for (;;) {
    const candidate = path.join(directory, "config.ncl");
    if (fs.existsSync(candidate)) {
      return candidate;
    }
    const parent = path.dirname(directory);
    if (parent === directory) {
      throw new Error(`Could not find config.ncl from ${process.cwd()}`);
    }
    directory = parent;
  }
};

const nickelBinary = () => {
  const serverBinary = "/home/fabio/.local/bin/nickel";
  return process.platform !== "win32" && fs.existsSync(serverBinary)
    ? serverBinary
    : "nickel";
};

const loadConfig = (configPath) => {
  const json = configPath.endsWith(".json")
    ? fs.readFileSync(configPath, "utf8")
    : execFileSync(nickelBinary(), ["export", configPath], {
        encoding: "utf8",
        timeout: 5000,
        windowsHide: true,
      });
  return validateConfig(JSON.parse(json));
};

const getConfigPath = () => cachedPath ?? findConfigPath();

const getWibbleConfig = () => {
  const configPath = getConfigPath();
  let mtimeMs;
  try {
    mtimeMs = fs.statSync(configPath).mtimeMs;
  } catch (error) {
    if (!cache) throw error;
    if (failedMtimeMs !== -2) {
      console.error("Failed to reload config.ncl; keeping last valid config", {
        error: error instanceof Error ? error.message : String(error),
      });
    }
    failedMtimeMs = -2;
    return cache;
  }
  if (cache && cachedMtimeMs === mtimeMs) {
    return cache;
  }
  if (cache && failedMtimeMs === mtimeMs) {
    return cache;
  }

  try {
    const next = loadConfig(configPath);
    cache = next;
    cachedPath = configPath;
    cachedMtimeMs = mtimeMs;
    failedMtimeMs = -1;
    return next;
  } catch (error) {
    failedMtimeMs = mtimeMs;
    if (cache) {
      console.error("Failed to reload config.ncl; keeping last valid config", {
        error: error instanceof Error ? error.message : String(error),
      });
      return cache;
    }
    throw error;
  }
};

const resetConfigCacheForTests = () => {
  cache = null;
  cachedPath = null;
  cachedMtimeMs = -1;
  failedMtimeMs = -1;
};

module.exports = {
  getWibbleConfig,
  getConfigPath,
  resetConfigCacheForTests,
};
