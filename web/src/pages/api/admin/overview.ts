import type { NextApiRequest, NextApiResponse } from "next";
import prisma from "@/core/PrismaWibble";
import { requireSession } from "@/core/serverSession";
import { Prisma } from "@prisma/client";

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
    translationStatuses,
    totalTranslationRows,
    recentTranslationJobs,
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
    prisma.$queryRaw<Array<{ status: string; count: bigint }>>(Prisma.sql`
      SELECT status, COUNT(*)::bigint AS count
      FROM translation_job
      GROUP BY status
    `),
    prisma.$queryRaw<Array<{ count: bigint }>>(Prisma.sql`
      SELECT COUNT(*)::bigint AS count FROM content_translation
    `),
    prisma.$queryRaw<
      Array<{
        id: string;
        slug: string;
        title: string;
        language_code: string;
        status: string;
        attempts: number;
        created_at: Date;
        updated_at: Date;
        last_error: string | null;
      }>
    >(Prisma.sql`
      SELECT j.id, c.slug, c.title, j.language_code, j.status, j.attempts,
             j.created_at, j.updated_at, j.last_error
      FROM translation_job j
      JOIN content c ON c.id = j.content_id
      ORDER BY j.updated_at DESC
      LIMIT 8
    `),
  ]);

  const images = imageStatuses.reduce<Record<string, number>>(
    (totals, item) => ({ ...totals, [item.status]: item._count._all }),
    {}
  );
  const translations = translationStatuses.reduce<Record<string, number>>(
    (totals, item) => ({ ...totals, [item.status]: Number(item.count) }),
    { pending: 0, processing: 0, completed: 0, failed: 0 }
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
    translations: { ...translations, total: Number(totalTranslationRows[0]?.count ?? 0) },
    recentTranslationJobs,
  });
}
