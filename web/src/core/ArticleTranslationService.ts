import type { WriterConfig } from "../../../config-runtime";
import { getWibbleConfig } from "../../../config-runtime";
import { ContentGenerator, type ArticleTranslation } from "@/worker/ContentGenerator";
import {
  ContentRepository,
  TranslationGenerationRateLimitError,
} from "./ContentRepository";
import {
  languageLabel,
  resolveSupportedTranslationLanguage,
} from "./translationLanguages";

type ArticleRecord = {
  id: string;
  title: string;
  description: string;
  content: string | null;
  model: string;
  published: boolean;
  flagged: boolean;
};

export type TranslationRecord = ArticleTranslation & {
  id: string;
  content_id: string;
  language_code: string;
  model: string;
  created_at: Date;
};

type TranslationRepository = {
  getContent(slug: string): Promise<ArticleRecord | null>;
  getTranslation(contentId: string, languageCode: string): Promise<TranslationRecord | null>;
  runTranslationGeneration(
    contentId: string,
    languageCode: string,
    userEmail: string,
    model: string,
    generate: () => Promise<ArticleTranslation>
  ): Promise<GenerationResult>;
  getTranslationLanguages(contentId: string): Promise<string[]>;
};

type TranslationGenerator = {
  generateTranslation(
    writer: WriterConfig,
    article: ArticleTranslation,
    targetLanguage: string,
    safetyIdentifier?: string
  ): Promise<ArticleTranslation>;
};

export class ArticleTranslationError extends Error {
  constructor(
    message: string,
    public readonly statusCode: number,
    public readonly retryAfterSeconds?: number
  ) {
    super(message);
    this.name = "ArticleTranslationError";
  }
}

type Dependencies = {
  repository: TranslationRepository;
  generator: TranslationGenerator;
  writers: WriterConfig[];
};

type GenerationResult = {
  translation: TranslationRecord;
  created: boolean;
};

const inFlightTranslations = new Map<string, Promise<GenerationResult>>();

export class ArticleTranslationService {
  private readonly repository: TranslationRepository;
  private readonly generator: TranslationGenerator;
  private readonly writers: WriterConfig[];

  constructor(dependencies?: Partial<Dependencies>) {
    this.repository = dependencies?.repository ?? new ContentRepository();
    this.generator = dependencies?.generator ?? new ContentGenerator();
    this.writers = dependencies?.writers ?? getWibbleConfig().generation.writers;
  }

  async availableLanguages(slug: string): Promise<string[]> {
    const article = await this.requireArticle(slug);
    return this.repository.getTranslationLanguages(article.id);
  }

  async generate(
    slug: string,
    requestedLanguage: string,
    userEmail: string,
    safetyIdentifier?: string
  ): Promise<GenerationResult> {
    const languageCode = resolveSupportedTranslationLanguage(requestedLanguage);
    const article = await this.requireArticle(slug);
    const existing = await this.repository.getTranslation(article.id, languageCode);
    if (existing) return { translation: existing, created: false };

    const key = `${article.id}\u0000${languageCode}\u0000${userEmail.toLowerCase()}`;
    const running = inFlightTranslations.get(key);
    if (running) return running;

    const generation = this.generateNewTranslation(
      article,
      languageCode,
      userEmail,
      safetyIdentifier
    );
    inFlightTranslations.set(key, generation);
    try {
      return await generation;
    } finally {
      if (inFlightTranslations.get(key) === generation) {
        inFlightTranslations.delete(key);
      }
    }
  }

  private async generateNewTranslation(
    article: ArticleRecord,
    languageCode: string,
    userEmail: string,
    safetyIdentifier?: string
  ): Promise<GenerationResult> {
    const writer =
      this.writers.find(
        (candidate) =>
          candidate.slug === article.model &&
          candidate.available &&
          !candidate.admin_only
      ) ?? this.writers.find((candidate) => candidate.available && !candidate.admin_only);
    if (!writer) {
      throw new ArticleTranslationError("No translation writer is available", 503);
    }

    const source = {
      title: article.title,
      description: article.description,
      content: article.content as string,
    };
    const targetLanguage = languageLabel(languageCode, "en");
    try {
      return await this.repository.runTranslationGeneration(
        article.id,
        languageCode,
        userEmail,
        writer.slug,
        () =>
          safetyIdentifier
            ? this.generator.generateTranslation(
                writer,
                source,
                targetLanguage,
                safetyIdentifier
              )
            : this.generator.generateTranslation(writer, source, targetLanguage)
      );
    } catch (error) {
      if (error instanceof TranslationGenerationRateLimitError) {
        throw new ArticleTranslationError(
          error.message,
          429,
          error.retryAfterSeconds
        );
      }
      throw error;
    }
  }

  private async requireArticle(slug: string): Promise<ArticleRecord> {
    const article = await this.repository.getContent(slug.trim().toLowerCase());
    if (
      !article ||
      article.flagged ||
      !article.published ||
      !article.content
    ) {
      throw new ArticleTranslationError("Article not found", 404);
    }
    return article;
  }
}
