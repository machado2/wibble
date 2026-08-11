import {
  ContentModerationError,
  ExternalServiceError,
  InvalidGptResponseError,
  RateLimitError,
} from "@/core/errors";
import prisma from "@/core/PrismaWibble";
import {
  getWibbleConfig,
  type WibbleConfig,
  type WriterConfig,
} from "../../../config-runtime";
import slugify from "slugify";
import { v4 as uuidv4 } from "uuid";

export type TitleDescription = {
  slug: string;
  title: string;
  description: string;
};

function escapeMarkdownString(str: string): string {
  const mdSpecialCharacters = [
    "`",
    "*",
    "_",
    "{",
    "}",
    "[",
    "]",
    "(",
    ")",
    "#",
    "+",
    "-",
    ".",
    "!",
    "|",
  ];
  let escapedString = "";

  for (const char of str) {
    escapedString += mdSpecialCharacters.includes(char) ? `\\${char}` : char;
  }
  return escapedString;
}

const responseText = (response: any): string | null => {
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

const providerApiKey = (config: WibbleConfig, writer: WriterConfig): string => {
  const key =
    writer.provider === "openrouter"
      ? config.secrets.openrouter_api_key.trim()
      : config.secrets.openai_api_key.trim();
  if (!key) {
    throw new ExternalServiceError(
      `Missing API key for writer ${writer.nickname}`
    );
  }
  return key;
};

export class ContentGenerator {
  async checkSlug(slug: string): Promise<boolean> {
    const response = await prisma.content.findUnique({
      where: { slug },
      select: { id: true },
    });
    return response != null;
  }

  async createSlug(title: string) {
    let slug = slugify(title, { lower: true, strict: true });
    if (slug.length < 1) {
      slug = uuidv4();
    }
    if (!(await this.checkSlug(slug))) {
      return slug;
    }
    for (let i = 0; ; i++) {
      const candidate = `${slug}-${i}`;
      if (!(await this.checkSlug(candidate))) {
        return candidate;
      }
    }
  }

  async askGpt(
    writer: WriterConfig,
    prompt: string,
    safetyIdentifier?: string,
    jsonOutput = false
  ): Promise<string> {
    if (!(await this.moderateContent(prompt))) {
      throw new ContentModerationError();
    }

    const config = getWibbleConfig();
    const apiKey = providerApiKey(config, writer);
    const apiUrl =
      writer.provider === "openrouter"
        ? config.generation.openrouter_api_url
        : config.generation.openai_api_url;
    const messages = [{ role: "user", content: prompt }];
    const request =
      writer.provider === "openrouter"
        ? {
            model: writer.slug,
            messages,
            max_tokens: config.generation.max_output_tokens,
            ...(safetyIdentifier ? { user: safetyIdentifier } : {}),
            ...(jsonOutput ? { response_format: { type: "json_object" } } : {}),
          }
        : {
            model: writer.slug,
            input: messages,
            reasoning: { effort: "none" },
            max_output_tokens: config.generation.max_output_tokens,
            ...(safetyIdentifier
              ? { safety_identifier: safetyIdentifier }
              : {}),
          };

    const response = await fetch(apiUrl, {
      method: "POST",
      headers: {
        Authorization: `Bearer ${apiKey}`,
        "Content-Type": "application/json",
        ...(writer.provider === "openrouter"
          ? {
              "HTTP-Referer": config.app.site_url,
              "X-OpenRouter-Title": "The Wibble",
            }
          : {}),
      },
      body: JSON.stringify(request),
    });
    const payload = await response.json().catch(() => null);
    if (response.status === 429) {
      throw new RateLimitError();
    }
    if (!response.ok) {
      throw new ExternalServiceError(
        `Language model request failed with HTTP ${response.status}`
      );
    }
    const content = responseText(payload);
    if (content == null) {
      throw new ExternalServiceError();
    }
    console.info("Language model request completed", {
      writer: writer.id,
      model: writer.slug,
      provider: writer.provider,
      promptLength: prompt.length,
      responseLength: content.length,
    });
    return content;
  }

  wrapUserInput(what: string, text: string | null) {
    if (!text) {
      return "";
    }
    const sanitized = escapeMarkdownString(text);
    return `This is the ${what}:\n\n: \`\`\`\n${sanitized}\`\`\`\n\n`;
  }

  async generateTitleAndDescription(
    writer: WriterConfig,
    suggestion: string | null,
    safetyIdentifier?: string
  ): Promise<TitleDescription> {
    const prompt = `You are a professional content writer for "The Wibble", a satirical news website that generates
    articles.

    Your task is to create a title and description for an article
    following the instructions provided from a user.

    * The description shouldn't spoil the article, don't spoil the punchline or any other surprise in the description and title.
    * The description should be introductory, not a summary of the article. Something like the first sentence of the article.
    * If there is a twist in the article, don't even hint it at the description.
    * Show, don't tell. Don't use the words like "satire", "irony" or "humor".

    ${this.wrapUserInput("user instructions", suggestion)}

    Your response must be a JSON object, following strictly the format:

    {"title": "Title of the article", "description": "Description of the article"}

    Important that your response must be only a JSON, as it needs to be parsed by the system.
    Any other text will be considered invalid.
    `;

    const generated = await this.askGpt(writer, prompt, safetyIdentifier, true);
    try {
      const parsed = JSON.parse(generated);
      if (
        typeof parsed?.title !== "string" ||
        typeof parsed?.description !== "string"
      ) {
        throw new Error("Missing title or description");
      }
      return {
        slug: await this.createSlug(parsed.title),
        title: parsed.title,
        description: parsed.description,
      };
    } catch {
      throw new InvalidGptResponseError();
    }
  }

  async moderateContent(content: string): Promise<boolean> {
    const config = getWibbleConfig();
    if (!config.generation.moderation_enabled) {
      return true;
    }
    const apiKey = config.secrets.openai_api_key.trim();
    if (!apiKey) {
      throw new ExternalServiceError(
        "Moderation requires secrets.openai_api_key in config.ncl"
      );
    }
    const response = await fetch(config.generation.moderation_api_url, {
      method: "POST",
      headers: {
        Authorization: `Bearer ${apiKey}`,
        "Content-Type": "application/json",
      },
      body: JSON.stringify({ model: "omni-moderation-latest", input: content }),
    });
    const payload = await response.json().catch(() => null);
    if (response.status === 429) {
      throw new RateLimitError();
    }
    if (!response.ok) {
      throw new ExternalServiceError(
        `OpenAI moderation failed with HTTP ${response.status}`
      );
    }
    const flagged = payload?.results?.[0]?.flagged;
    if (typeof flagged !== "boolean") {
      throw new ExternalServiceError("Invalid OpenAI moderation response");
    }
    if (flagged) {
      console.info("Content was rejected by moderation", {
        contentLength: content.length,
      });
    }
    return !flagged;
  }
}
