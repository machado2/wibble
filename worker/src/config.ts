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

export const Config = {
  gpt4MinCoolDownHours: numberConfig("GPT4_MIN_COOLDOWN_HOURS", 20),
  gpt4MaxCoolDownHours: numberConfig("GPT4_MAX_COOLDOWN_HOURS", 28),
  coolDownSec: numberConfig("COOL_DOWN_SEC", 10),
  idleSleepSec: numberConfig("IDLE_SLEEP_SEC", 10),
  gpt3DailyLimit: numberConfig("GPT3_DAILY_LIMIT", 1000),
  waitOnErrorSec: numberConfig("WAIT_ON_ERROR_SEC", 60),
  openAiApiKey: strConfig("OPENAI_API_KEY"),
  openAiApiUrl: strConfig(
    "OPENAI_API_URL",
    "https://api.openai.com/v1/responses"
  ),
  databaseUrl: strConfig("DATABASE_URL"),
  textModel: strConfig(
    "LANGUAGE_MODEL",
    strConfig("OPENAI_MODEL", strConfig("TEXT_MODEL", "gpt-3.5-turbo"))
  ),
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
