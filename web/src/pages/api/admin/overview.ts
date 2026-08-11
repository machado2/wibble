import type { NextApiRequest, NextApiResponse } from "next";
import prisma from "@/core/PrismaWibble";
import { requireSession } from "@/core/serverSession";

export default async function handler(
  req: NextApiRequest,
  res: NextApiResponse
) {
  if (!(await requireSession(req, res))) return;
  if (req.method !== "GET") {
    res.status(405).json({ error: "Method not allowed" });
    return;
  }

  const last24Hours = new Date(Date.now() - 24 * 60 * 60 * 1000);
  const [
    totalArticles,
    publishedArticles,
    generatingArticles,
    flaggedArticles,
    unresolvedUrls,
    searchesLast24Hours,
    imageStatuses,
    recentArticles,
  ] = await Promise.all([
    prisma.content.count(),
    prisma.content.count({ where: { published: true, flagged: false } }),
    prisma.content.count({ where: { generating: true } }),
    prisma.content.count({ where: { flagged: true } }),
    prisma.not_found_request.count({ where: { generated_at: null } }),
    prisma.search_history.count({
      where: { created_at: { gte: last24Hours } },
    }),
    prisma.image_cache.groupBy({
      by: ["status"],
      _count: { _all: true },
    }),
    prisma.content.findMany({
      orderBy: { created_at: "desc" },
      take: 6,
      select: {
        id: true,
        slug: true,
        title: true,
        created_at: true,
        generating: true,
        flagged: true,
        published: true,
        votes: true,
        view_count: true,
      },
    }),
  ]);

  const images = imageStatuses.reduce<Record<string, number>>(
    (totals, item) => ({ ...totals, [item.status]: item._count._all }),
    {}
  );

  res.status(200).json({
    totalArticles,
    publishedArticles,
    generatingArticles,
    flaggedArticles,
    unresolvedUrls,
    searchesLast24Hours,
    images,
    recentArticles,
  });
}
