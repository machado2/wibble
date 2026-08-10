import { NextApiRequest, NextApiResponse } from "next";
import { Prisma } from "@prisma/client";
import prisma from "@/core/PrismaWibble";
import { DateTime } from "luxon";

const parseInteger = (
  value: string | string[] | undefined
): number | undefined => {
  if (!value) return 0;
  const parsed = parseInt(value as string);
  if (isNaN(parsed)) return 0;
  return parsed;
};

const parseString = (s: string | string[] | undefined): string | undefined => {
  if (!s) return undefined;
  if (Array.isArray(s)) return s[0];
  return s as string;
};

export type ImageListItem = {
  id: string;
  prompt: string;
  created_at: string;
};

const list = async (
  afterId: string | undefined,
  sort: string | undefined,
  days: number | undefined,
  page_size: number | undefined,
  search: string | undefined,
  model?: string
): Promise<ImageListItem[]> => {
  if (!page_size || page_size < 1 || page_size > 100) {
    page_size = 30;
  }

  const afterContent = afterId
    ? await prisma.image_cache.findUnique({ where: { id: afterId } })
    : undefined;

  let additionalFilter = {};

  if (afterContent) {
    if (sort == "most_viewed") {
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
  if (sort == "most_viewed") {
    orderBy.push({ view_count: "desc" });
  }
  orderBy.push({ created_at: "desc" });

  let searchFilter: Prisma.image_cacheWhereInput | undefined = undefined;
  if (search) {
    searchFilter = {
      OR: [{ prompt: { search: search } }, { model: { search: search } }],
    };
  }

  const contents = await prisma.image_cache.findMany({
    select: {
      id: true,
      prompt: true,
      created_at: true,
    },
    where: {
      flagged: false,
      model,
      created_at: days
        ? { gte: DateTime.utc().minus({ days }).toJSDate() }
        : undefined,
      AND: additionalFilter,
      ...searchFilter,
    },
    orderBy,
    take: page_size,
  });
  return contents.map((i) => {
    return {
      id: i.id,
      prompt: i.prompt,
      created_at: i.created_at.toISOString(),
    };
  });
};

export default async function handler(
  req: NextApiRequest,
  res: NextApiResponse
) {
  try {
    const { query } = req;
    const period = (query.t as string) ?? undefined;
    const days = [undefined, 7, 30][["week", "month"].indexOf(period) + 1];
    const data = await list(
      parseString(query.afterId),
      parseString(query.sort),
      days,
      parseInteger(query.pageSize),
      parseString(query.search),
      parseString(query.model)
    );

    res.status(200).json(data);
  } catch (error) {
    console.error(error);
    res.status(500).json({ message: "Error reading latest news" });
  }
}
