import {
  ContentModerationError,
  ExternalServiceError,
  RateLimitError,
} from "./errors";
import logger from "./logger";
import {
  Config,
  getWibbleConfig,
  languageApiKey,
  writerForModel,
} from "./config";
import type { WriterProvider } from "../../config-runtime";

type Message = { role: "system" | "user"; content: string };

const responsesText = (response: any): string | null => {
  const chatText = response?.choices?.[0]?.message?.content;
  if (typeof chatText === "string" && chatText) {
    return chatText;
  }

  if (typeof response?.output_text === "string" && response.output_text) {
    return response.output_text;
  }

  const text = response?.output
    ?.filter((item: any) => item?.type === "message")
    ?.flatMap((item: any) => item?.content ?? [])
    ?.filter((item: any) => item?.type === "output_text")
    ?.map((item: any) => item?.text ?? "")
    ?.join("");
  return text || null;
};

class TextGenerator {
  private async post(
    url: string,
    body: unknown,
    apiKey: string,
    provider?: WriterProvider
  ): Promise<any> {
    const isOpenRouterLanguageRequest = provider === "openrouter";
    const response = await fetch(url, {
      method: "POST",
      headers: {
        Authorization: `Bearer ${apiKey}`,
        "Content-Type": "application/json",
        ...(isOpenRouterLanguageRequest
          ? {
              "HTTP-Referer": Config.siteUrl,
              "X-OpenRouter-Title": "The Wibble",
            }
          : {}),
      },
      body: JSON.stringify(body),
    });
    const payload = await response.json().catch(() => null);
    if (response.status === 429) {
      throw new RateLimitError();
    }
    if (!response.ok) {
      throw new ExternalServiceError(
        `External model request failed with HTTP ${response.status}`
      );
    }
    return payload;
  }

  async moderateContent(content: string): Promise<boolean> {
    if (!Config.moderationEnabled) {
      return true;
    }
    const moderationApiKey = Config.moderationApiKey;
    if (!moderationApiKey) {
      throw new ExternalServiceError(
        "Moderation requires secrets.openai_api_key in config.ncl"
      );
    }
    const response = await this.post(
      Config.moderationApiUrl,
      { model: "omni-moderation-latest", input: content },
      moderationApiKey
    );
    const moderation_result = response?.results?.[0]?.flagged;
    if (typeof moderation_result !== "boolean") {
      throw new ExternalServiceError("Invalid OpenAI moderation response");
    }
    if (moderation_result) {
      logger.info(`Content rejected by moderation (${content.length} chars)`);
    }
    return !moderation_result;
  }

  async askGpt(
    model: string,
    prompt: string,
    systemMessage?: string,
    safetyIdentifier?: string
  ): Promise<string> {
    logger.info(`Requesting ${model} generation (${prompt.length} chars)`);
    if (!(await this.moderateContent(prompt))) {
      throw new ContentModerationError();
    }

    const messages: Message[] = [];
    if (systemMessage) {
      messages.push({ role: "system", content: systemMessage });
    }
    messages.push({ role: "user", content: prompt });

    const writer = writerForModel(model);
    const config = getWibbleConfig();
    const apiUrl =
      writer.provider === "openrouter"
        ? config.generation.openrouter_api_url
        : config.generation.openai_api_url;

    const request =
      writer.provider === "openrouter"
        ? {
            model,
            messages,
            max_tokens: Config.maxOutputTokens,
            ...(safetyIdentifier ? { user: safetyIdentifier } : {}),
          }
        : {
            model,
            input: messages,
            reasoning: { effort: "none" },
            max_output_tokens: Config.maxOutputTokens,
            ...(safetyIdentifier
              ? { safety_identifier: safetyIdentifier }
              : {}),
          };
    const response = await this.post(
      apiUrl,
      request,
      languageApiKey(writer.provider),
      writer.provider
    );
    const content = responsesText(response);
    if (content == null) {
      logger.error(`content is null`);
      throw new ExternalServiceError();
    }
    logger.info(`Generation completed (${content.length} chars)`);
    return content;
  }
}

export default new TextGenerator();
