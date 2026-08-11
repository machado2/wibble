import dotenv from "dotenv";
import { createRequire } from "node:module";
import type {
  WibbleConfig,
  WriterConfig,
  WriterProvider,
} from "../../config-runtime";

dotenv.config();

const require = createRequire(import.meta.url);
const { getWibbleConfig } = require("../../config-runtime/index.cjs") as {
  getWibbleConfig: () => WibbleConfig;
};

const optionalSecret = (key: string) => process.env[key]?.trim() || undefined;

const requiredSecret = (key: string) => {
  const value = optionalSecret(key);
  if (!value) {
    throw new Error(`Missing required environment variable ${key}`);
  }
  return value;
};

export const writerForModel = (model: string): WriterConfig => {
  const configured = getWibbleConfig().generation.writers.find(
    (writer) => writer.slug === model
  );
  if (configured) {
    return configured;
  }
  const provider: WriterProvider = model.includes("/")
    ? "openrouter"
    : "openai";
  return {
    id: `legacy-${model}`,
    nickname: model,
    slug: model,
    provider,
    available: false,
    admin_only: true,
  };
};

export const languageApiKey = (provider: WriterProvider): string =>
  requiredSecret(
    provider === "openrouter" ? "OPENROUTER_API_KEY" : "OPENAI_API_KEY"
  );

export const Config = {
  get coolDownSec() {
    return getWibbleConfig().worker.cool_down_seconds;
  },
  get idleSleepSec() {
    return getWibbleConfig().worker.idle_sleep_seconds;
  },
  get waitOnErrorSec() {
    return getWibbleConfig().worker.wait_on_error_seconds;
  },
  get updateScoresIntervalSec() {
    return getWibbleConfig().worker.update_scores_interval_seconds;
  },
  get moderationEnabled() {
    return getWibbleConfig().generation.moderation_enabled;
  },
  get moderationApiKey() {
    return optionalSecret("OPENAI_API_KEY") ?? optionalSecret("OPENAI_KEY");
  },
  get moderationApiUrl() {
    return getWibbleConfig().generation.moderation_api_url;
  },
  get maxOutputTokens() {
    return getWibbleConfig().generation.max_output_tokens;
  },
  get siteUrl() {
    return getWibbleConfig().app.site_url;
  },
  get imageMode() {
    return getWibbleConfig().image.mode;
  },
  get imagesDir() {
    return getWibbleConfig().app.images_dir;
  },
  get replicateApiToken() {
    return requiredSecret("REPLICATE_API_TOKEN");
  },
  get replicateApiUrl() {
    return getWibbleConfig().image.replicate_api_url;
  },
  get replicateMinRequestIntervalSeconds() {
    return getWibbleConfig().image.minimum_request_interval_seconds;
  },
};

export { getWibbleConfig };
