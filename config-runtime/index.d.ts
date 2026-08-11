export type WriterProvider = "openai" | "openrouter";

export type WriterConfig = {
  id: string;
  nickname: string;
  slug: string;
  provider: WriterProvider;
  available: boolean;
  admin_only: boolean;
};

export type WibbleConfig = {
  secrets: {
    database_url: string;
    nextauth_secret: string;
    sso_client_secret: string;
    admin_email: string;
    safety_identifier_secret: string;
    openai_api_key: string;
    openrouter_api_key: string;
    replicate_api_token: string;
    loggly_token: string;
  };
  app: {
    site_url: string;
    sso_public_url: string;
    discord_url: string;
    images_dir: string;
  };
  auth: {
    sso_issuer_url: string;
    sso_client_id: string;
  };
  generation: {
    writers: WriterConfig[];
    openai_api_url: string;
    openrouter_api_url: string;
    moderation_enabled: boolean;
    moderation_api_url: string;
    max_output_tokens: number;
    max_prompt_length: number;
  };
  image: {
    mode: string;
    replicate_api_url: string;
    minimum_request_interval_seconds: number;
  };
  worker: {
    cool_down_seconds: number;
    idle_sleep_seconds: number;
    wait_on_error_seconds: number;
    update_scores_interval_seconds: number;
  };
  logging: {
    loggly_subdomain: string;
    web_tags: string[];
    worker_tags: string[];
  };
};

export function getWibbleConfig(): WibbleConfig;
export function getConfigPath(): string;
export function resetConfigCacheForTests(): void;
