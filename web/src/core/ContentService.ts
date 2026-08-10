import { ContentRepository, ContentWithCurrentVote } from "./ContentRepository";
import { RateLimiterMemory } from "rate-limiter-flexible";
import { content } from "@prisma/client";
import { MDXRemoteSerializeResult } from "next-mdx-remote";
import { makeRandomExcuse } from "@/components/ExcuseMaker";
import { serialize } from "next-mdx-remote/serialize";
import { NewsListItem } from "./NewsListItem";
import { dontWaitFor } from "./dontWaitFor";
import { ContentGenerator } from "@/worker/ContentGenerator";
import { DateTime } from "luxon";
import modelSelector from "./ModelSelector";

export type ParsedResponse = {
  id?: string;
  content: MDXRemoteSerializeResult;
  imageUrl: string | null;
  titleInContent: boolean;
  loading: boolean;
  datetime: string;
  votes?: number;
  currentVote?: number;
};

export class ContentService {
  private repository = new ContentRepository();
  private generator = new ContentGenerator();

  private globallimiter = new RateLimiterMemory({
    points: 1000,
    duration: 3600,
  });

  async mdxMessage(msg: string, loading: boolean): Promise<ParsedResponse> {
    const mdxContent = `---
title: ${msg}
description: ${msg}
---`;
    const mdxData = await serialize(mdxContent, { parseFrontmatter: true });
    return {
      content: mdxData,
      imageUrl: null,
      loading,
      titleInContent: false,
      datetime: new Date().toUTCString(),
    };
  }

  async fatalErrorResponse(): Promise<ParsedResponse> {
    return this.mdxMessage(makeRandomExcuse(), false);
  }

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
      datetime: new Date().toUTCString(),
    };
  }

  async parseResponse(content: ContentWithCurrentVote): Promise<ParsedResponse> {
    const mdxSource = this.addFrontMatter(
      content.title,
      content.description,
      content.content as string
    );
    const mdxData = await serialize(mdxSource, { parseFrontmatter: true });
    return {
      id: content.id,
      content: mdxData,
      imageUrl: content.image_id ? `/api/image/${content.image_id}` : null,
      loading: false,
      titleInContent: content.title_repeated ?? false,
      datetime: DateTime.fromJSDate(content.created_at).toISO()!,
      votes: content.votes,
      currentVote: content.votesRelation && content.votesRelation.length > 0 ? (content.votesRelation[0].downvote ? -1 : 1) : 0,
    };
  }

  isValidSlug(slug: string): boolean {
    const pattern = /^[a-z0-9_-]+$/;
    return pattern.test(slug);
  }

  public async processSlug(
    email: string | null,
    pslug: string
  ): Promise<ParsedResponse> {
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
      return await this.fatalErrorResponse();
    }

    let content = await this.repository.getContent(slug, email ?? undefined);
    if (content?.flagged) {
      return this.fatalErrorResponse();
    }
    if (content === null) {
      if (!this.isValidSlug(slug)) {
        return await this.fatalErrorResponse();
      }
      await this.globallimiter.consume("global", 1);
      const model = await modelSelector.selectNextModel();
      const titleDescription = await this.generator.generateTitleAndDescription(model, slug);
      if (!titleDescription) {
        return await this.fatalErrorResponse();
      }
      content = await this.repository.createContent(
        model,
        titleDescription.title,
        slug,
        titleDescription.description,
        slug,
        email,
      );
    }
    if (content.content === null) {
      return await this.loadingResponse(content.title, content.description);
    }
    dontWaitFor(this.repository.incrementContentViewCount(content.id));
    return this.parseResponse(content);
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
    model?: string
  ): Promise<content> {
    if (!model) {
      model = await modelSelector.selectNextModel();
    }
    const titleDescription = await this.generator.generateTitleAndDescription(
      model,
      suggestion
    );

    return await this.repository.createContent(
      model,
      titleDescription.title,
      titleDescription.slug,
      titleDescription.description,
      suggestion,
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
