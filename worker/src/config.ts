import dotenv from "dotenv";

dotenv.config();

const numberConfig = (key: string, def?: number) => {
  const value = process.env[key];
  if (!value) {
    if (def === undefined) {
      throw new Error(`Missing required environment variable ${key}`);
    }
    return def;
  }
  return parseInt(value);
};

const strConfig = (key: string, def?: string) => {
  const value = process.env[key];
  if (!value) {
    if (def === undefined) {
      throw new Error(`Missing required environment variable ${key}`);
    }
    return def;
  }
  return value;
};

const optionalStrConfig = (key: string) =>
  process.env[key]?.trim() || undefined;

const booleanConfig = (key: string, def = false) => {
  const value = optionalStrConfig(key);
  if (value === undefined) {
    return def;
  }
  return ["1", "true", "yes", "on"].includes(value.toLowerCase());
};

const openRouterApiKey = optionalStrConfig("OPENROUTER_API_KEY");
const openAiApiKey =
  optionalStrConfig("OPENAI_API_KEY") ?? optionalStrConfig("OPENAI_KEY");
const languageProvider = openRouterApiKey ? "openrouter" : "openai";
const languageApiKey = openRouterApiKey ?? openAiApiKey;

if (!languageApiKey) {
  throw new Error(
    "Missing required environment variable OPENROUTER_API_KEY or OPENAI_API_KEY"
  );
}

export const Config = {
  coolDownSec: numberConfig("COOL_DOWN_SEC", 10),
  idleSleepSec: numberConfig("IDLE_SLEEP_SEC", 10),
  waitOnErrorSec: numberConfig("WAIT_ON_ERROR_SEC", 60),
  languageProvider,
  languageApiKey,
  languageApiUrl:
    optionalStrConfig("LANGUAGE_API_URL") ??
    (languageProvider === "openrouter"
      ? "https://openrouter.ai/api/v1/chat/completions"
      : optionalStrConfig("OPENAI_API_URL") ??
        "https://api.openai.com/v1/responses"),
  moderationEnabled: booleanConfig("OPENAI_MODERATION_ENABLED", false),
  moderationApiKey: openAiApiKey,
  moderationApiUrl:
    optionalStrConfig("OPENAI_MODERATION_API_URL") ??
    "https://api.openai.com/v1/moderations",
  siteUrl: strConfig("SITE_URL", "https://wibble.fbmac.net"),
  databaseUrl: strConfig("DATABASE_URL"),
  imageMode: strConfig("IMAGE_MODE", "replicate"),
  replicateApiToken: strConfig("REPLICATE_API_TOKEN"),
  replicateApiUrl: strConfig(
    "REPLICATE_API_URL",
    "https://api.replicate.com/v1/models/black-forest-labs/flux-schnell/predictions"
  ),
  replicateMinRequestIntervalSeconds: Math.max(
    1,
    numberConfig("REPLICATE_MIN_REQUEST_INTERVAL_SECONDS", 30)
  ),
  logglyToken: strConfig("LOGGLY_TOKEN", ""),
};
