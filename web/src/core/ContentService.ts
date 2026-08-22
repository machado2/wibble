import { ContentRepository, ContentWithCurrentVote } from "./ContentRepository";
import { content } from "@prisma/client";
import { MDXRemoteSerializeResult } from "next-mdx-remote";
import { serialize } from "next-mdx-remote/serialize";
import { NewsListItem } from "./NewsListItem";
import { dontWaitFor } from "./dontWaitFor";
import { ContentGenerator } from "@/worker/ContentGenerator";
import { DateTime } from "luxon";
import type { WriterConfig } from "../../../config-runtime";
import { resolveSupportedTranslationLanguage } from "./translationLanguages";

type TranslationView = {
  title: string;
  description: string;
  content: string;
  language_code: string;
};

export type ParsedResponse = {
  id?: string;
  content: MDXRemoteSerializeResult;
  imageUrl: string | null;
  titleInContent: boolean;
  loading: boolean;
  datetime: string;
  votes?: number;
  currentVote?: number;
  languageCode: string | null;
  availableLanguages: string[];
};

export class ContentService {
  private repository = new ContentRepository();
  private generator = new ContentGenerator();

  addFrontMatter(title: string, description: string, text: string): string {
    if (text.startsWith("---")) {
      return text;
    }
    const esctitle = title.replace(/'/g, "''");
    const escdesc = description.replace(/'/g, "''");
    return `---
title: '${esctitle}'
description: '${escdesc}'
---
${text}`;
  }

  async loadingResponse(
    title: string,
    description: string
  ): Promise<ParsedResponse> {
    const markdown = this.addFrontMatter(
      title,
      description,
      `${description}\n\nContent is being created. Please wait a few minutes and refresh the page.`
    );

    const mdxData = await serialize(markdown, { parseFrontmatter: true });
    return {
      content: mdxData,
      imageUrl: null,
      loading: true,
      titleInContent: false,
      datetime: new Date().toISOString(),
      languageCode: null,
      availableLanguages: [],
    };
  }

  async parseResponse(
    content: ContentWithCurrentVote,
    translation: TranslationView | null = null,
    availableLanguages: string[] = []
  ): Promise<ParsedResponse> {
    const view = translation ?? content;
    const mdxSource = this.addFrontMatter(
      view.title,
      view.description,
      view.content as string
    );
    const mdxData = await serialize(mdxSource, { parseFrontmatter: true });
    return {
      id: content.id,
      content: mdxData,
      imageUrl: content.image_id ? `/api/image/${content.image_id}` : null,
      loading: false,
      titleInContent: false,
      datetime: DateTime.fromJSDate(content.created_at).toISO()!,
      votes: content.votes,
      currentVote:
        content.votesRelation && content.votesRelation.length > 0
          ? content.votesRelation[0].downvote
            ? -1
            : 1
          : 0,
      languageCode: translation?.language_code ?? null,
      availableLanguages,
    };
  }

  public async processSlug(
    email: string | null,
    pslug: string,
    requestedLanguage?: string
  ): Promise<ParsedResponse | null> {
    const slug = pslug.trim().toLowerCase();

    // only printable characters accepted
    if (
      !slug ||
      slug.length == 0 ||
      ![...slug].every((c) => {
        const code = c.charCodeAt(0);
        return (
          (code >= 32 && code <= 126) || // ASCII printable characters
          (code >= 161 && code <= 55295) || // Latin-1 Supplement to High Surrogates
          (code >= 57344 && code <= 65533) || // Private Use Area to Non-Character code points
          (code >= 65536 && code <= 1114111)
        ); // Unicode Supplementary Planes
      })
    ) {
      return null;
    }

    let content = await this.repository.getContent(slug, email ?? undefined);
    if (content?.flagged) {
      return null;
    }
    if (content === null) {
      return null;
    }
    if (content.content === null) {
      return await this.loadingResponse(content.title, content.description);
    }
    let languageCode: string | null = null;
    if (requestedLanguage) {
      try {
        languageCode = resolveSupportedTranslationLanguage(requestedLanguage);
      } catch {
        languageCode = null;
      }
    }
    const [availableLanguages, translation] = await Promise.all([
      this.repository.getTranslationLanguages(content.id),
      languageCode
        ? this.repository.getTranslation(content.id, languageCode)
        : Promise.resolve(null),
    ]);
    dontWaitFor(this.repository.incrementContentViewCount(content.id));
    return this.parseResponse(content, translation, availableLanguages);
  }

  parseNewsListItem(content: ContentWithCurrentVote): NewsListItem {
    let currentVote = 0;
    if (content.votesRelation && content.votesRelation.length > 0) {
      currentVote = content.votesRelation[0].downvote ? -1 : 1;
    }
    return {
      id: content.id,
      title: content.title as string,
      description: content.description as string,
      imagePrompt: content.image_prompt ?? "",
      slug: content.slug,
      created_at: content.created_at.toISOString(),
      votes: content.votes,
      currentVote,
      hotScore: content.hot_score ?? 0,
    };
  }

  isValidForTitle(s: string): boolean {
    // Check if the string contains at least one alphabetic character
    let containsAlpha = /[a-zA-Z]/.test(s);

    // Check if the string contains only allowed characters: alphanumeric, spaces, hyphens, and underscores
    let allowedCharacters = /^[a-zA-Z0-9 _-]*$/.test(s);

    // Check if the string is of minimum length 3
    let minLength = s.length >= 3;

    return containsAlpha && allowedCharacters && minLength;
  }

  async generateForSuggestion(
    email: string | null,
    suggestion: string,
    writer: WriterConfig,
    requestedSlug?: string,
    safetyIdentifier?: string
  ): Promise<content> {
    const titleDescription = await this.generator.generateTitleAndDescription(
      writer,
      suggestion,
      safetyIdentifier
    );

    const storedUserInput = JSON.stringify({
      version: 1,
      suggestion,
      ...(safetyIdentifier ? { safetyIdentifier } : {}),
    });

    return await this.repository.createContent(
      writer.slug,
      titleDescription.title,
      requestedSlug ?? titleDescription.slug,
      titleDescription.description,
      storedUserInput,
      email
    );
  }

  async vote(contentId: string, email: string, downvote: boolean) {
    await this.repository.vote(contentId, email, downvote);
  }

  async unvote(contentId: string, email: string) {
    await this.repository.unvote(contentId, email);
  }
}
