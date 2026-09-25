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

export type ArticleTranslation = {
  title: string;
  description: string;
  content: string;
};

const MAX_TRANSLATED_TITLE_LENGTH = 500;
const MAX_TRANSLATED_DESCRIPTION_LENGTH = 20_000;
const MAX_TRANSLATED_CONTENT_LENGTH = 2_000_000;

const PRESERVE_MARKER = /\{\{WIBBLE_PRESERVE_\d+\}\}/g;

type MarkdownLink = {
  start: number;
  end: number;
  url: string | null;
  image: boolean;
  referenceId: string | null;
};

type ProtectedRange = { start: number; end: number };

const isEscaped = (text: string, index: number): boolean => {
  let backslashes = 0;
  for (let cursor = index - 1; cursor >= 0 && text[cursor] === "\\"; cursor -= 1) {
    backslashes += 1;
  }
  return backslashes % 2 === 1;
};

const findBalanced = (
  text: string,
  start: number,
  opening: string,
  closing: string
): number => {
  let depth = 0;
  for (let cursor = start; cursor < text.length; cursor += 1) {
    if (isEscaped(text, cursor)) continue;
    if (text[cursor] === opening) depth += 1;
    if (text[cursor] === closing) {
      depth -= 1;
      if (depth === 0) return cursor;
    }
  }
  return -1;
};

const destinationFrom = (raw: string): string => {
  const value = raw.trim();
  if (value.startsWith("<")) {
    const end = value.indexOf(">");
    return end >= 0 ? value.slice(1, end) : value;
  }
  let nested = 0;
  for (let cursor = 0; cursor < value.length; cursor += 1) {
    if (isEscaped(value, cursor)) continue;
    if (value[cursor] === "(") nested += 1;
    if (value[cursor] === ")" && nested > 0) nested -= 1;
    if (/\s/.test(value[cursor]) && nested === 0) return value.slice(0, cursor);
  }
  return value;
};

const scanMarkdownLinks = (text: string): MarkdownLink[] => {
  const links: MarkdownLink[] = [];
  for (let cursor = 0; cursor < text.length; cursor += 1) {
    const image = text[cursor] === "!" && text[cursor + 1] === "[";
    const opening = image ? cursor + 1 : cursor;
    if (text[opening] !== "[" || isEscaped(text, opening)) continue;
    const labelEnd = findBalanced(text, opening, "[", "]");
    if (labelEnd < 0) continue;
    let next = labelEnd + 1;
    while (text[next] === " " || text[next] === "\t") next += 1;
    if (text[next] === "(") {
      const destinationEnd = findBalanced(text, next, "(", ")");
      if (destinationEnd < 0) continue;
      links.push({
        start: image ? cursor : opening,
        end: destinationEnd + 1,
        url: destinationFrom(text.slice(next + 1, destinationEnd)),
        image,
        referenceId: null,
      });
      cursor = destinationEnd;
      continue;
    }
    if (image && text[next] === "[") {
      const referenceEnd = findBalanced(text, next, "[", "]");
      if (referenceEnd < 0) continue;
      const label = text.slice(opening + 1, labelEnd);
      const reference = text.slice(next + 1, referenceEnd).trim() || label;
      links.push({
        start: cursor,
        end: referenceEnd + 1,
        url: null,
        image: true,
        referenceId: reference.trim().toLowerCase(),
      });
      cursor = referenceEnd;
      continue;
    }
    if (image) {
      const label = text.slice(opening + 1, labelEnd);
      links.push({
        start: cursor,
        end: labelEnd + 1,
        url: null,
        image: true,
        referenceId: label.trim().toLowerCase(),
      });
      cursor = labelEnd;
    }
  }
  return links;
};

const scanDefinitions = (text: string) => {
  const definitions: Array<ProtectedRange & { id: string; url: string }> = [];
  const pattern = /^( {0,3})\[((?:\\.|[^\]\\\r\n])+)\]:[ \t]*(.*)$/gm;
  let match: RegExpExecArray | null;
  while ((match = pattern.exec(text)) !== null) {
    let rawDestination = match[3];
    let end = match.index + match[0].length;
    if (!rawDestination.trim()) {
      const continuation = text
        .slice(pattern.lastIndex)
        .match(/^\r?\n[ \t]{0,4}(\S[^\r\n]*)/);
      if (continuation) {
        rawDestination = continuation[1];
        end = pattern.lastIndex + continuation[0].length;
      }
    }
    definitions.push({
      start: match.index,
      end,
      id: match[2].trim().toLowerCase(),
      url: destinationFrom(rawDestination),
    });
  }
  return definitions;
};

const assertSafeGeneratedImageDirective = (directive: string) => {
  const attributes = directive.slice("<GeneratedImage".length, -2);
  const allowed = new Set(["prompt", "alt", "position", "cssClass"]);
  const seen = new Set<string>();
  let cursor = 0;
  while (cursor < attributes.length) {
    while (/\s/.test(attributes[cursor] ?? "")) cursor += 1;
    if (cursor >= attributes.length) break;
    const name = attributes.slice(cursor).match(/^([A-Za-z][A-Za-z0-9_-]*)/)?.[1];
    if (!name || !allowed.has(name) || seen.has(name)) {
      throw new InvalidGptResponseError();
    }
    seen.add(name);
    cursor += name.length;
    while (/\s/.test(attributes[cursor] ?? "")) cursor += 1;
    if (attributes[cursor] !== "=") throw new InvalidGptResponseError();
    cursor += 1;
    while (/\s/.test(attributes[cursor] ?? "")) cursor += 1;
    const quote = attributes[cursor];
    if (quote !== '"' && quote !== "'") throw new InvalidGptResponseError();
    cursor += 1;
    const valueStart = cursor;
    while (
      cursor < attributes.length &&
      (attributes[cursor] !== quote || isEscaped(attributes, cursor))
    ) {
      cursor += 1;
    }
    if (cursor >= attributes.length || cursor === valueStart) {
      throw new InvalidGptResponseError();
    }
    cursor += 1;
  }
  if (!seen.has("prompt")) throw new InvalidGptResponseError();
};

const scanGeneratedImages = (text: string): ProtectedRange[] => {
  const ranges: ProtectedRange[] = [];
  let searchFrom = 0;
  while (true) {
    const start = text.indexOf("<GeneratedImage", searchFrom);
    if (start < 0) return ranges;
    let quote: string | null = null;
    let end = -1;
    for (let cursor = start + 15; cursor < text.length - 1; cursor += 1) {
      const char = text[cursor];
      if (quote) {
        if (char === quote && !isEscaped(text, cursor)) quote = null;
        continue;
      }
      if (char === '"' || char === "'") {
        quote = char;
        continue;
      }
      if (char === "/" && text[cursor + 1] === ">") {
        end = cursor + 2;
        break;
      }
      if (char === ">") throw new InvalidGptResponseError();
    }
    if (end < 0 || quote) throw new InvalidGptResponseError();
    const directive = text.slice(start, end);
    assertSafeGeneratedImageDirective(directive);
    ranges.push({ start, end });
    searchFrom = end;
  }
};

const decodeProtocolEntities = (value: string): string =>
  value
    .replace(/&#(?:x([0-9a-f]+)|([0-9]+));?/gi, (_entity, hex, decimal) => {
      const codePoint = Number.parseInt(hex ?? decimal, hex ? 16 : 10);
      try {
        return String.fromCodePoint(codePoint);
      } catch {
        return "";
      }
    })
    .replace(/&(colon|tab|newline);?/gi, (_entity, name: string) => {
      const characters: Record<string, string> = {
        colon: ":",
        tab: "\t",
        newline: "\n",
      };
      return characters[name.toLowerCase()];
    });

const assertSafeUrl = (value: unknown, image: boolean) => {
  if (typeof value !== "string") throw new InvalidGptResponseError();
  let normalized = value.trim();
  for (let pass = 0; pass < 2; pass += 1) {
    try {
      normalized = decodeURIComponent(normalized);
    } catch {
      break;
    }
  }
  normalized = decodeProtocolEntities(normalized).replace(
    /[\\\u0000-\u0020\u007f-\u009f]/g,
    ""
  );
  const protocol = normalized.match(/^([A-Za-z][A-Za-z0-9+.-]*):/)?.[1]?.toLowerCase();
  const allowed = image ? ["http", "https"] : ["http", "https", "mailto"];
  if (protocol && !allowed.includes(protocol)) throw new InvalidGptResponseError();
};

const assertNoRawMdx = (content: string) => {
  const withoutMarkers = content.replace(PRESERVE_MARKER, "WIBBLE_PRESERVED");
  // `-->` is intentionally allowed: the MDX compiler treats a bare arrow as
  // inert text (it only matters inside `<!-- ... -->`, whose opener is still
  // rejected here), and rejecting it broke translations of harmless code spans
  // such as `toaster --> cityhall`.
  if (
    /<\/?[A-Za-z][^>]*>|<!--|[{}]/.test(withoutMarkers) ||
    /^\s*(?:import\s.+\sfrom\s+|export\s+(?:default|const|let|var|function|class|\{))/m.test(
      withoutMarkers
    )
  ) {
    throw new InvalidGptResponseError();
  }
};

const assertSafeTranslatedMarkdown = (content: string) => {
  assertNoRawMdx(content);
  for (const link of scanMarkdownLinks(content.replace(PRESERVE_MARKER, "SAFE"))) {
    if (link.image || link.url === null) throw new InvalidGptResponseError();
    assertSafeUrl(link.url, false);
  }
  for (const definition of scanDefinitions(content)) {
    assertSafeUrl(definition.url, false);
  }
};

const protectArticleDirectives = (content: string) => {
  const links = scanMarkdownLinks(content);
  const imageReferences = new Set(
    links.flatMap((link) =>
      link.image && link.referenceId ? [link.referenceId] : []
    )
  );
  const definitions = scanDefinitions(content);
  const ranges: ProtectedRange[] = [
    ...scanGeneratedImages(content),
    ...links.filter((link) => link.image).map(({ start, end }) => ({ start, end })),
    ...definitions
      .filter((definition) => imageReferences.has(definition.id))
      .map(({ start, end }) => ({ start, end })),
  ];
  for (const link of links) {
    if (link.url !== null) assertSafeUrl(link.url, link.image);
  }
  for (const definition of definitions) {
    assertSafeUrl(definition.url, imageReferences.has(definition.id));
  }

  ranges.sort((left, right) => left.start - right.start);
  const directives: string[] = [];
  let cursor = 0;
  let protectedContent = "";
  for (const range of ranges) {
    if (range.start < cursor) continue;
    protectedContent += content.slice(cursor, range.start);
    protectedContent += `{{WIBBLE_PRESERVE_${directives.length}}}`;
    directives.push(content.slice(range.start, range.end));
    cursor = range.end;
  }
  protectedContent += content.slice(cursor);
  assertSafeTranslatedMarkdown(protectedContent);
  return { protectedContent, directives };
};

const restoreArticleDirectives = (content: string, directives: string[]): string => {
  assertSafeTranslatedMarkdown(content);
  let restored = content;
  directives.forEach((directive, index) => {
    const marker = `{{WIBBLE_PRESERVE_${index}}}`;
    if (restored.split(marker).length !== 2) {
      throw new InvalidGptResponseError();
    }
    restored = restored.replace(marker, directive);
  });
  if (/\{\{WIBBLE_PRESERVE_\d+\}\}/.test(restored)) {
    throw new InvalidGptResponseError();
  }
  return restored;
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
            // deepseek-v4-flash defaults to high reasoning effort on OpenRouter,
            // which can burn the whole output budget before any content comes
            // back (finish_reason "length" with content: null).
            reasoning: { enabled: false },
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
      throw new ExternalServiceError(
        "Language model returned no usable content"
      );
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

  async generateTranslation(
    writer: WriterConfig,
    article: ArticleTranslation,
    targetLanguage: string,
    safetyIdentifier?: string
  ): Promise<ArticleTranslation> {
    const { protectedContent, directives } = protectArticleDirectives(
      article.content
    );
    const source = JSON.stringify({
      title: article.title,
      description: article.description,
      content: protectedContent,
    });
    const prompt = `Translate this satirical news article into ${targetLanguage}.

Preserve its tone, jokes, meaning, Markdown formatting, paragraph structure, names, and factual details.
Tokens shaped like {{WIBBLE_PRESERVE_N}} are immutable article directives: copy each one exactly once and do not translate, edit, move, add, or remove it.
Return only a JSON object with exactly these string fields: title, description, content.
Do not add commentary or Markdown fences around the JSON.

Source article JSON:
${source}`;
    const generated = await this.askGpt(
      writer,
      prompt,
      safetyIdentifier,
      true
    );
    try {
      const parsed = JSON.parse(generated);
      if (
        typeof parsed?.title !== "string" ||
        typeof parsed?.description !== "string" ||
        typeof parsed?.content !== "string" ||
        !parsed.title.trim() ||
        !parsed.description.trim() ||
        !parsed.content.trim() ||
        parsed.title.trim().length > MAX_TRANSLATED_TITLE_LENGTH ||
        parsed.description.trim().length > MAX_TRANSLATED_DESCRIPTION_LENGTH ||
        parsed.content.length > MAX_TRANSLATED_CONTENT_LENGTH
      ) {
        throw new InvalidGptResponseError();
      }
      return {
        title: parsed.title.trim(),
        description: parsed.description.trim(),
        content: restoreArticleDirectives(parsed.content, directives),
      };
    } catch (error) {
      if (error instanceof InvalidGptResponseError) throw error;
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
