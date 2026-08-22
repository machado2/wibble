import type { NextApiRequest, NextApiResponse } from "next";
import prisma from "@/core/PrismaWibble";
import { requireSession } from "@/core/serverSession";
import { Prisma } from "@prisma/client";
import {
  TRANSLATION_LIMIT_PER_WINDOW,
  TRANSLATION_LIMIT_WINDOW_MS,
} from "@/core/ContentRepository";
import { BACKGROUND_TRANSLATION_IDENTITY } from "@/core/TranslationQueueService";
import { buildTranslationOperations } from "@/core/translationOperations";

export default async function handler(
  req: NextApiRequest,
  res: NextApiResponse
) {
  if (!(await requireSession(req, res))) return;
  if (req.method !== "GET") {
    res.status(405).json({ error: "Method not allowed" });
    return;
  }

  const now = new Date();
  const last24Hours = new Date(now.getTime() - 24 * 60 * 60 * 1000);
  const translationWindowStart = new Date(now.getTime() - TRANSLATION_LIMIT_WINDOW_MS);
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
    translationAttempts,
    translationQueueTiming,
    translationLanguages,
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
      WHERE requested_by = ${BACKGROUND_TRANSLATION_IDENTITY}
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
        next_attempt_at: Date;
        started_at: Date | null;
        completed_at: Date | null;
        last_error: string | null;
      }>
    >(Prisma.sql`
      SELECT j.id, c.slug, c.title, j.language_code, j.status, j.attempts,
             j.created_at, j.updated_at, j.next_attempt_at, j.started_at,
             j.completed_at, j.last_error
      FROM translation_job j
      JOIN content c ON c.id = j.content_id
      WHERE j.requested_by = ${BACKGROUND_TRANSLATION_IDENTITY}
      ORDER BY j.updated_at DESC
      LIMIT 8
    `),
    prisma.$queryRaw<Array<{ created_at: Date }>>(Prisma.sql`
      SELECT created_at
      FROM translation_generation_attempt
      WHERE user_email = ${BACKGROUND_TRANSLATION_IDENTITY}
        AND created_at >= ${translationWindowStart}
      ORDER BY created_at ASC
    `),
    prisma.$queryRaw<
      Array<{
        waiting_for_quota: bigint;
        oldest_pending_at: Date | null;
        next_attempt_at: Date | null;
        completed_last_hour: bigint;
        completed_last_24_hours: bigint;
        average_duration_seconds: number | null;
      }>
    >(Prisma.sql`
      SELECT
        COUNT(*) FILTER (
          WHERE status = 'pending'
            AND last_error = 'Limite temporário de traduções'
        )::bigint AS waiting_for_quota,
        MIN(created_at) FILTER (WHERE status = 'pending') AS oldest_pending_at,
        MIN(next_attempt_at) FILTER (WHERE status = 'pending') AS next_attempt_at,
        COUNT(*) FILTER (
          WHERE status = 'completed'
            AND completed_at >= ${translationWindowStart}
        )::bigint AS completed_last_hour,
        COUNT(*) FILTER (
          WHERE status = 'completed'
            AND completed_at >= ${last24Hours}
        )::bigint AS completed_last_24_hours,
        AVG(EXTRACT(EPOCH FROM (completed_at - started_at))) FILTER (
          WHERE status = 'completed'
            AND started_at IS NOT NULL
            AND completed_at IS NOT NULL
        )::double precision AS average_duration_seconds
      FROM translation_job
      WHERE requested_by = ${BACKGROUND_TRANSLATION_IDENTITY}
    `),
    prisma.$queryRaw<
      Array<{
        language_code: string;
        translated: bigint;
        pending: bigint;
        processing: bigint;
        completed: bigint;
        failed: bigint;
        waiting_for_quota: bigint;
      }>
    >(Prisma.sql`
      WITH languages AS (
        SELECT language_code
        FROM translation_job
        WHERE requested_by = ${BACKGROUND_TRANSLATION_IDENTITY}
        UNION
        SELECT translations.language_code
        FROM content_translation translations
        JOIN content translated_content
          ON translated_content.id = translations.content_id
        WHERE translated_content.published = true
          AND translated_content.flagged = false
          AND translated_content.content IS NOT NULL
      )
      SELECT
        languages.language_code,
        COUNT(DISTINCT translations.content_id)::bigint AS translated,
        COUNT(DISTINCT jobs.id) FILTER (WHERE jobs.status = 'pending')::bigint AS pending,
        COUNT(DISTINCT jobs.id) FILTER (WHERE jobs.status = 'processing')::bigint AS processing,
        COUNT(DISTINCT jobs.id) FILTER (WHERE jobs.status = 'completed')::bigint AS completed,
        COUNT(DISTINCT jobs.id) FILTER (WHERE jobs.status = 'failed')::bigint AS failed,
        COUNT(DISTINCT jobs.id) FILTER (
          WHERE jobs.status = 'pending'
            AND jobs.last_error = 'Limite temporário de traduções'
        )::bigint AS waiting_for_quota
      FROM languages
      LEFT JOIN (
        SELECT translations.content_id, translations.language_code
        FROM content_translation translations
        JOIN content translated_content
          ON translated_content.id = translations.content_id
        WHERE translated_content.published = true
          AND translated_content.flagged = false
          AND translated_content.content IS NOT NULL
      ) translations ON translations.language_code = languages.language_code
      LEFT JOIN translation_job jobs
        ON jobs.language_code = languages.language_code
       AND jobs.requested_by = ${BACKGROUND_TRANSLATION_IDENTITY}
      GROUP BY languages.language_code
      ORDER BY translated DESC, languages.language_code ASC
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
  const queueTiming = translationQueueTiming[0];
  const translationOperations = buildTranslationOperations({
    now,
    limit: TRANSLATION_LIMIT_PER_WINDOW,
    windowMs: TRANSLATION_LIMIT_WINDOW_MS,
    eligibleArticles: publishedArticles,
    attempts: translationAttempts.map((attempt) => attempt.created_at),
    statusCounts: {
      pending: translations.pending,
      processing: translations.processing,
      completed: translations.completed,
      failed: translations.failed,
    },
    waitingForQuota: Number(queueTiming?.waiting_for_quota ?? 0),
    oldestPendingAt: queueTiming?.oldest_pending_at ?? null,
    nextAttemptAt: queueTiming?.next_attempt_at ?? null,
    completedLastHour: Number(queueTiming?.completed_last_hour ?? 0),
    completedLast24Hours: Number(queueTiming?.completed_last_24_hours ?? 0),
    averageDurationSeconds: queueTiming?.average_duration_seconds ?? null,
    languages: translationLanguages.map((language) => ({
      languageCode: language.language_code,
      translated: Number(language.translated),
      pending: Number(language.pending),
      processing: Number(language.processing),
      completed: Number(language.completed),
      failed: Number(language.failed),
      waitingForQuota: Number(language.waiting_for_quota),
    })),
  });

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
    translationOperations,
    recentTranslationJobs,
  });
}
