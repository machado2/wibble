import {
  ContentModerationError,
  ExternalServiceError,
  RateLimitError,
} from "./errors";
import logger from "./logger";
import { Config } from "./config";

type Message = { role: "system" | "user"; content: string };

const responsesText = (response: any): string | null => {
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
  private async post(url: string, body: unknown): Promise<any> {
    const response = await fetch(url, {
      method: "POST",
      headers: {
        Authorization: `Bearer ${Config.openAiApiKey}`,
        "Content-Type": "application/json",
      },
      body: JSON.stringify(body),
    });
    const payload = await response.json().catch(() => null);
    if (response.status === 429) {
      throw new RateLimitError();
    }
    if (!response.ok) {
      throw new ExternalServiceError(
        `OpenAI request failed with HTTP ${response.status}`
      );
    }
    return payload;
  }

  async moderateContent(content: string): Promise<boolean> {
    const moderationUrl = Config.openAiApiUrl.replace(
      /\/responses?\/?$/,
      "/moderations"
    );
    const response = await this.post(moderationUrl, { input: content });
    const moderation_result = response?.results?.[0]?.flagged === true;
    if (moderation_result) {
      logger.info(`Content rejected by moderation (${content.length} chars)`);
    }
    return !moderation_result;
  }

  async askGpt(
    model: string,
    prompt: string,
    systemMessage?: string
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

    const response = await this.post(Config.openAiApiUrl, {
      model,
      input: messages,
      reasoning: { effort: "none" },
      max_output_tokens: 16000,
    });
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
