import { Prisma, content, content_vote } from "@prisma/client";
import { DateTime } from "luxon";
import { v4 as uuidv4 } from "uuid";
import logger from "./logger";

import prisma from "./PrismaWibble";

export type ContentWithCurrentVote = content & {
  votesRelation?: content_vote[];
};

export class ContentRepository {
  async registerContentGenerationFailure(slug: string, reason: string, exceptionText?: string, content?: string) {
    logger.info(
      `Registering content generation failure for ${slug}, reason: ${reason}`
    );
    const id = uuidv4();
    await prisma.history_generation_fail.create({
      data: {
        id,
        slug,
        created_at: DateTime.utc().toJSDate(),
        reason,
        exception : exceptionText,
        content
      },
    });
  }

  async getPagedContent(
    last_hot_score: number | undefined,
    last_created_at: Date | undefined,
    page_size: number,
    search: string | undefined | null,
    userEmail: string | null | undefined
  ): Promise<ContentWithCurrentVote[]> {
    const includeVotesRelation = userEmail
      ? { votesRelation: { where: { user_email: userEmail } } }
      : undefined;

    let additionalFilter = {};
    if (last_hot_score !== undefined && last_created_at !== undefined) {
      const epsilon = 1e-17; // Small value to tolerate the floating point imprecision

      additionalFilter = {
        OR: [
          {
            hot_score: { lt: last_hot_score - epsilon },
          },
          {
            hot_score: { lte: last_hot_score + epsilon },
            created_at: { lt: last_created_at },
          },
        ],
      };
    }

    return await prisma.content.findMany({
      where: {
        ...additionalFilter,
        flagged: false,
        title: search ? { contains: search } : undefined,
      },
      orderBy: [{ hot_score: "desc" }, { created_at: "desc" }],
      take: page_size,
      include: includeVotesRelation,
    });
  }

  async getContent(slug: string): Promise<content | null> {
    return await prisma.content.findUnique({
      where: { slug },
    });
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
      orderBy: [{ votes: "desc" }, { created_at: "asc" }],
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
    imagePrompt: string,
    timeEllapsedMs: number
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
          title_repeated: containsDuplicatedTitle,
          image_id: imageId,
          image_prompt: imagePrompt,
          generation_time_ms: timeEllapsedMs,
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
      if (failCount > 5) {
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
