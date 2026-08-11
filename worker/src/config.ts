import { createRequire } from "node:module";
import type {
  WibbleConfig,
  WriterConfig,
  WriterProvider,
} from "../../config-runtime";

const require = createRequire(import.meta.url);
const { getWibbleConfig } = require("../../config-runtime/index.cjs") as {
  getWibbleConfig: () => WibbleConfig;
};

const requiredSecret = (value: string, field: string) => {
  const normalized = value.trim();
  if (!normalized) {
    throw new Error(`Missing required Nickel configuration ${field}`);
  }
  return normalized;
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
  provider === "openrouter"
    ? requiredSecret(
        getWibbleConfig().secrets.openrouter_api_key,
        "secrets.openrouter_api_key"
      )
    : requiredSecret(
        getWibbleConfig().secrets.openai_api_key,
        "secrets.openai_api_key"
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
    return getWibbleConfig().secrets.openai_api_key.trim() || undefined;
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
    return requiredSecret(
      getWibbleConfig().secrets.replicate_api_token,
      "secrets.replicate_api_token"
    );
  },
  get replicateApiUrl() {
    return getWibbleConfig().image.replicate_api_url;
  },
  get replicateMinRequestIntervalSeconds() {
    return getWibbleConfig().image.minimum_request_interval_seconds;
  },
};

export { getWibbleConfig };
