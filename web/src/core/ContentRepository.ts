import { Prisma, content, content_vote } from "@prisma/client";
import { DateTime } from "luxon";
import { v4 as uuidv4 } from "uuid";
import prisma from "./PrismaWibble";
import { NewsListItem } from "./NewsListItem";

export type ContentWithCurrentVote = content & {
  votesRelation?: content_vote[];
};

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
    model?: string
  ): Promise<NewsListItem[]> {
    if (!page_size || page_size < 1 || page_size > 100) {
      page_size = 20;
    }

    const includeVotesRelation = userEmail
      ? { votesRelation: { where: { user_email: userEmail } } }
      : undefined;

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
      searchFilter = { OR:
        [
          { slug: { search: search } },
          { title: { search: search } },
          { description: { search: search } },
          { content: { search: search } },
        ]
      };
    }

    const contents = await prisma.content.findMany({
      where: {
        flagged: false,
        model,
        created_at: days ? { gte: DateTime.utc().minus({ days }).toJSDate() } : undefined,
        AND: additionalFilter,
        ...searchFilter,
      },
      orderBy,
      take: page_size,
      include: includeVotesRelation,
    });
    return contents.map(this.parseNewsListItem);
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
      },
    });
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
          title_repeated: containsDuplicatedTitle,
          image_id: imageId,
          image_prompt: imagePrompt,
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
