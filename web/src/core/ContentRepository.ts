import {
  Prisma,
  content,
  content_translation,
  content_vote,
} from "@prisma/client";
import { DateTime } from "luxon";
import { v4 as uuidv4 } from "uuid";
import prisma from "./PrismaWibble";
import { NewsListItem } from "./NewsListItem";
import { isAutomaticTranslationLanguage } from "./translationLanguages";

export type ContentWithCurrentVote = content & {
  votesRelation?: content_vote[];
  translations?: Pick<content_translation, "title" | "description">[];
  translationJobs?: Array<{ status: string }>;
};

export class TranslationGenerationRateLimitError extends Error {
  constructor(public readonly retryAfterSeconds: number) {
    super("Translation generation limit reached");
    this.name = "TranslationGenerationRateLimitError";
  }
}

const TRANSLATION_LIMIT_WINDOW_MS = 60 * 60 * 1000;
const TRANSLATION_LIMIT_PER_WINDOW = 10;

export class ContentRepository {
  async registerContentGenerationFailure(slug: string, reason: string) {
    console.log(
      `Registering content generation failure for ${slug}, reason: ${reason}`
    );
    const id = uuidv4();
    await prisma.history_generation_fail.create({
      data: {
        id,
        slug,
        created_at: DateTime.utc().toJSDate(),
        reason,
      },
    });
  }

  async getNextPage(
    afterId: string | undefined,
    sort: string | undefined,
    days: number | undefined,
    page_size: number | undefined,
    search: string | undefined,
    userEmail: string | undefined,
    model?: string,
    languageCode?: string
  ): Promise<NewsListItem[]> {
    if (!page_size || page_size < 1 || page_size > 100) {
      page_size = 20;
    }

    const translationRequested = Boolean(
      languageCode && isAutomaticTranslationLanguage(languageCode)
    );
    const include = {
      ...(userEmail
        ? { votesRelation: { where: { user_email: userEmail } } }
        : {}),
      ...(languageCode
        ? {
            translations: {
              where: { language_code: languageCode },
              select: { title: true, description: true },
              take: 1,
            },
          }
        : {}),
      ...(translationRequested
        ? {
            translationJobs: {
              where: { language_code: languageCode },
              select: { status: true },
              take: 1,
            },
          }
        : {}),
    };

    const afterContent = afterId
      ? await prisma.content.findUnique({ where: { id: afterId } })
      : undefined;

    let additionalFilter = {};

    if (afterContent) {
      if (sort == "hot") {
        additionalFilter = {
          OR: [
            {
              hot_score: { lt: afterContent.hot_score },
            },
            {
              hot_score: afterContent.hot_score,
              created_at: { lt: afterContent.created_at },
            },
          ],
        };
      } else if (sort == "most_voted") {
        additionalFilter = {
          OR: [
            {
              votes: { lt: afterContent.votes },
            },
            {
              votes: afterContent.votes,
              created_at: { lt: afterContent.created_at },
            },
          ],
        };
      } else if (sort == "most_viewed") {
        additionalFilter = {
          OR: [
            {
              view_count: { lt: afterContent.view_count },
            },
            {
              view_count: afterContent.view_count,
              created_at: { lt: afterContent.created_at },
            },
          ],
        };
      } else {
        additionalFilter = {
          created_at: { lt: afterContent.created_at },
        };
      }
    }

    const orderBy: any[] = [];
    if (sort == "hot") {
      orderBy.push({ hot_score: "desc" });
    } else if (sort == "most_voted") {
      orderBy.push({ votes: "desc" });
    } else if (sort == "most_viewed") {
      orderBy.push({ view_count: "desc" });
    }
    orderBy.push({ created_at: "desc" });

    let searchFilter: Prisma.contentWhereInput | undefined = undefined;
    if (search) {
      searchFilter = {
        OR: [
          { slug: { contains: search, mode: "insensitive" } },
          { title: { contains: search, mode: "insensitive" } },
          { description: { contains: search, mode: "insensitive" } },
          { content: { contains: search, mode: "insensitive" } },
        ],
      };
    }

    const contents = await prisma.content.findMany({
      where: {
        flagged: false,
        published: true,
        model,
        created_at: days ? { gte: DateTime.utc().minus({ days }).toJSDate() } : undefined,
        AND: additionalFilter,
        ...searchFilter,
      },
      orderBy,
      take: page_size,
      include,
    });
    return contents.map((content) =>
      this.parseNewsListItem(content, translationRequested)
    );
  }

  parseNewsListItem(
    content: ContentWithCurrentVote,
    translationRequested = false
  ): NewsListItem {
    let currentVote = 0;
    if (content.votesRelation && content.votesRelation.length > 0) {
      currentVote = content.votesRelation[0].downvote ? -1 : 1;
    }
    const translation = content.translations?.[0];
    const translationFailed = content.translationJobs?.[0]?.status === "failed";
    return {
      id: content.id,
      title: translation?.title ?? (content.title as string),
      description: translation?.description ?? (content.description as string),
      imagePrompt: content.image_prompt ?? "",
      slug: content.slug,
      created_at: content.created_at.toISOString(),
      votes: content.votes,
      currentVote,
      hotScore: content.hot_score ?? 0,
      translationState: translationRequested
        ? translation
          ? "translated"
          : translationFailed
            ? "failed"
            : "pending"
        : undefined,
    };
  }

  async getContent(
    slug: string,
    userEmail?: string
  ): Promise<ContentWithCurrentVote | null> {
    const includeVotesRelation = userEmail
      ? { votesRelation: { where: { user_email: userEmail } } }
      : undefined;
    return await prisma.content.findUnique({
      where: { slug },
      include: includeVotesRelation,
    });
  }

  async getTranslation(
    contentId: string,
    languageCode: string
  ): Promise<content_translation | null> {
    return prisma.content_translation.findUnique({
      where: {
        content_id_language_code: {
          content_id: contentId,
          language_code: languageCode,
        },
      },
    });
  }

  async getTranslationLanguages(contentId: string): Promise<string[]> {
    const translations = await prisma.content_translation.findMany({
      where: { content_id: contentId },
      select: { language_code: true },
      orderBy: { language_code: "asc" },
    });
    return translations.map((translation) => translation.language_code);
  }

  async runTranslationGeneration(
    contentId: string,
    languageCode: string,
    userEmail: string,
    model: string,
    generate: () => Promise<{ title: string; description: string; content: string }>
  ): Promise<{ translation: content_translation; created: boolean }> {
    const now = new Date();
    const cutoff = new Date(now.getTime() - TRANSLATION_LIMIT_WINDOW_MS);
    await prisma.translation_generation_attempt.deleteMany({
      where: { created_at: { lt: cutoff } },
    });

    return prisma.$transaction(
      async (tx) => {
        await tx.$queryRaw`SELECT 1::int AS locked FROM pg_advisory_xact_lock(hashtextextended(${`translation-user:${userEmail.toLowerCase()}`}, 0))`;
        await tx.$queryRaw`SELECT 1::int AS locked FROM pg_advisory_xact_lock(hashtextextended(${`translation:${contentId}:${languageCode}`}, 0))`;

        const existing = await tx.content_translation.findUnique({
          where: {
            content_id_language_code: {
              content_id: contentId,
              language_code: languageCode,
            },
          },
        });
        if (existing) return { translation: existing, created: false };

        const attempts = await tx.translation_generation_attempt.findMany({
          where: { user_email: userEmail.toLowerCase(), created_at: { gte: cutoff } },
          select: { created_at: true },
          orderBy: { created_at: "asc" },
        });
        if (attempts.length >= TRANSLATION_LIMIT_PER_WINDOW) {
          const retryAt =
            attempts[0].created_at.getTime() + TRANSLATION_LIMIT_WINDOW_MS;
          throw new TranslationGenerationRateLimitError(
            Math.max(1, Math.ceil((retryAt - now.getTime()) / 1000))
          );
        }

        // Persist the paid attempt on a separate autocommit connection before the
        // provider call. The surrounding advisory locks still serialize counting,
        // while provider failures or transaction rollbacks cannot erase the charge.
        await prisma.translation_generation_attempt.create({
          data: {
            id: uuidv4(),
            user_email: userEmail.toLowerCase(),
            content_id: contentId,
            language_code: languageCode,
          },
        });
        const translated = await generate();
        const translation = await tx.content_translation.create({
          data: {
            id: uuidv4(),
            content_id: contentId,
            language_code: languageCode,
            title: translated.title,
            description: translated.description,
            content: translated.content,
            model,
          },
        });
        return { translation, created: true };
      },
      { maxWait: 15_000, timeout: 120_000 }
    );
  }

  async incrementContentViewCount(id: string) {
    await prisma.content.update({
      where: { id },
      data: {
        view_count: {
          increment: 1,
        },
      },
    });
  }

  async createSearchHistory(searchTerm: string, resultCount: number) {
    const id = uuidv4();
    await prisma.search_history.create({
      data: {
        id,
        term: searchTerm,
        result_count: resultCount,
      },
    });
  }

  async getNextContentToGenerate(): Promise<content | null> {
    return await prisma.content.findFirst({
      where: {
        flagged: false,
        generating: true,
      },
      orderBy: {
        created_at: "asc",
      },
    });
  }

  async vote(contentId: string, email: string, downvote: boolean) {
    await prisma.$transaction(
      async (tx) => {
        const existingVote = await tx.content_vote.findUnique({
          where: {
            content_id_user_email: {
              content_id: contentId,
              user_email: email,
            },
          },
        });

        if (existingVote?.downvote === downvote) {
          return;
        }

        // Atomic update for content votes
        if (existingVote) {
          const voteDiff = downvote ? -2 : 2;
          await tx.content.update({
            where: {
              id: contentId,
            },
            data: {
              votes: {
                increment: voteDiff,
              },
            },
          });

          // Updating the existing vote
          await tx.content_vote.update({
            where: {
              content_id_user_email: {
                content_id: contentId,
                user_email: email,
              },
            },
            data: {
              downvote: downvote,
            },
          });
        } else {
          const voteDiff = downvote ? -1 : 1;
          await tx.content.update({
            where: {
              id: contentId,
            },
            data: {
              votes: {
                increment: voteDiff,
              },
            },
          });

          // Creating a new vote
          await tx.content_vote.create({
            data: {
              content_id: contentId,
              user_email: email,
              downvote: downvote,
            },
          });
        }
      },
      { isolationLevel: Prisma.TransactionIsolationLevel.Serializable }
    );
  }

  async unvote(contentId: string, email: string) {
    await prisma.$transaction(
      async (tx) => {
        const existingVote = await tx.content_vote.findUnique({
          where: {
            content_id_user_email: {
              content_id: contentId,
              user_email: email,
            },
          },
        });

        // Check if a vote by the user exists
        if (!existingVote) {
          return; // No vote to unvote
        }

        // Decrement or increment the content's vote count based on the user's vote
        const voteDiff = existingVote.downvote ? 1 : -1;
        await tx.content.update({
          where: {
            id: contentId,
          },
          data: {
            votes: {
              increment: voteDiff,
            },
          },
        });

        // Delete the user's vote from the content_vote table
        await tx.content_vote.delete({
          where: {
            content_id_user_email: {
              content_id: contentId,
              user_email: email,
            },
          },
        });
      },
      { isolationLevel: Prisma.TransactionIsolationLevel.Serializable }
    );
  }

  async createContent(
    model: string,
    title: string,
    slug: string,
    description: string,
    userInput: string,
    userEmail: string | null
  ): Promise<content> {
    const id = uuidv4();
    try {
      return await prisma.content.create({
        data: {
          id,
          slug,
          title,
          description,
          created_at: DateTime.utc().toJSDate(),
          generating: true,
          flagged: false,
          user_input: userInput,
          user_email: userEmail,
          model,
          published: false,
        },
      });
    } catch (error) {
      if (
        error instanceof Prisma.PrismaClientKnownRequestError &&
        error.code === "P2002"
      ) {
        const existing = await this.getContent(slug);
        if (existing) return existing;
      }
      throw error;
    }
  }

  async startTextGeneration(content: content) {
    await prisma.content.update({
      where: { id: content.id },
      data: {
        generation_started_at: DateTime.utc().toJSDate(),
      },
    });
  }

  async finishTextGeneration(
    slug: string,
    generated_content: string,
    model: string,
    prompt_version: number,
    containsDuplicatedTitle: boolean,
    imageId: string,
    imagePrompt: string
  ) {
    const content = await this.getContent(slug);
    if (content) {
      await prisma.content.update({
        where: { slug },
        data: {
          content: generated_content,
          generating: false,
          generation_finished_at: DateTime.utc().toJSDate(),
          model: model,
          prompt_version: prompt_version,
          image_id: imageId,
          image_prompt: imagePrompt,
          published: true,
        },
      });
    } else {
      console.error(`Content ${slug} not found`);
    }
  }

  async finishFlagged(slug: string) {
    const content = await this.getContent(slug);
    if (content) {
      await prisma.content.update({
        where: { slug },
        data: {
          generating: false,
          flagged: true,
          generation_finished_at: DateTime.utc().toJSDate(),
        },
      });
    }
  }

  async failGeneration(slug: string) {
    const content = await this.getContent(slug);
    if (content) {
      const failCount = (content.fail_count ?? 0) + 1;
      if (failCount > 10) {
        await this.finishFlagged(slug);
      } else {
        await prisma.content.update({
          where: { slug },
          data: {
            fail_count: failCount,
            created_at: DateTime.utc().toJSDate(),
          },
        });
      }
    }
  }

  async updateContent(slug: string, new_content: string) {
    const content = await this.getContent(slug);
    if (content) {
      await prisma.content.update({
        where: { slug },
        data: {
          content: {
            set: new_content,
          },
        },
      });
    }
  }

  async deleteContent(slug: string) {
    await prisma.content.delete({
      where: { slug },
    });
  }
}
