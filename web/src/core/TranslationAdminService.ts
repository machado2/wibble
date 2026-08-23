import { Prisma } from "@prisma/client";
import { v4 as uuidv4 } from "uuid";
import prisma from "./PrismaWibble";
import {
  assertTranslationHourlyLimit,
  BACKGROUND_TRANSLATION_IDENTITY,
  getTranslationQuotaPolicy,
  nextTranslationSlotAt,
  TRANSLATION_LIMIT_WINDOW_MS,
} from "./translationQuota";

export { MAX_TRANSLATION_HOURLY_LIMIT } from "./translationQuota";

export type TranslationQueueAdminJob = {
  id: string;
  contentId: string;
  slug: string;
  title: string;
  languageCode: string;
  status: "pending" | "processing";
  attempts: number;
  createdAt: Date;
  nextAttemptAt: Date;
  startedAt: Date | null;
  lastError: string | null;
};

export class TranslationAdminService {
  async snapshot() {
    const now = new Date();
    const windowCutoff = new Date(now.getTime() - TRANSLATION_LIMIT_WINDOW_MS);
    const policy = await getTranslationQuotaPolicy(prisma);
    const cutoff =
      policy.budgetResetAt && policy.budgetResetAt > windowCutoff
        ? policy.budgetResetAt
        : windowCutoff;
    const [attempts, jobs] = await Promise.all([
      prisma.$queryRaw<Array<{ created_at: Date }>>(Prisma.sql`
        SELECT created_at
        FROM translation_generation_attempt
        WHERE user_email = ${BACKGROUND_TRANSLATION_IDENTITY}
          AND created_at >= ${cutoff}
        ORDER BY created_at ASC, id ASC
      `),
      prisma.$queryRaw<
        Array<{
          id: string;
          content_id: string;
          slug: string;
          title: string;
          language_code: string;
          status: "pending" | "processing";
          attempts: number;
          created_at: Date;
          next_attempt_at: Date;
          started_at: Date | null;
          last_error: string | null;
        }>
      >(Prisma.sql`
        SELECT j.id, j.content_id, c.slug, c.title, j.language_code,
               j.status, j.attempts, j.created_at, j.next_attempt_at,
               j.started_at, j.last_error
        FROM translation_job j
        JOIN content c ON c.id = j.content_id
        WHERE j.requested_by = ${BACKGROUND_TRANSLATION_IDENTITY}
          AND j.status IN ('processing', 'pending')
        ORDER BY
          CASE WHEN j.status = 'processing' THEN 0 ELSE 1 END,
          CASE WHEN j.next_attempt_at <= CURRENT_TIMESTAMP THEN 0 ELSE 1 END,
          j.created_at ASC,
          j.id ASC
        LIMIT 200
      `),
    ]);
    const used = attempts.length;
    return {
      hourlyLimit: policy.hourlyLimit,
      used,
      remaining: Math.max(0, policy.hourlyLimit - used),
      windowMinutes: TRANSLATION_LIMIT_WINDOW_MS / 60_000,
      nextReleaseAt:
        policy.hourlyLimit > 0 && used >= policy.hourlyLimit
          ? nextTranslationSlotAt(
              attempts.map((attempt) => attempt.created_at),
              policy.hourlyLimit,
              TRANSLATION_LIMIT_WINDOW_MS
            )
          : null,
      jobs: jobs.map(
        (job): TranslationQueueAdminJob => ({
          id: job.id,
          contentId: job.content_id,
          slug: job.slug,
          title: job.title,
          languageCode: job.language_code,
          status: job.status,
          attempts: job.attempts,
          createdAt: job.created_at,
          nextAttemptAt: job.next_attempt_at,
          startedAt: job.started_at,
          lastError: job.last_error,
        })
      ),
    };
  }

  async updateHourlyLimit(hourlyLimit: number) {
    const value = assertTranslationHourlyLimit(hourlyLimit);
    await prisma.$transaction(async (tx) => {
      await tx.$queryRaw(Prisma.sql`
        SELECT 1::int AS locked
        FROM pg_advisory_xact_lock(
          hashtextextended(${`translation-user:${BACKGROUND_TRANSLATION_IDENTITY}`}, 0)
        )
      `);
      await tx.$executeRaw(Prisma.sql`
        INSERT INTO translation_runtime_setting (id, hourly_limit, updated_at)
        VALUES ('automatic', ${value}, CURRENT_TIMESTAMP)
        ON CONFLICT (id) DO UPDATE
          SET hourly_limit = EXCLUDED.hourly_limit,
              updated_at = CURRENT_TIMESTAMP
      `);
      await tx.$executeRaw(Prisma.sql`
        INSERT INTO translation_admin_event (id, action, job_id, numeric_value)
        VALUES (${uuidv4()}, 'quota_limit_changed', NULL, ${value})
      `);
    });
    return { hourlyLimit: value };
  }

  async resetBudget() {
    const resetAttempts = await prisma.$transaction(async (tx) => {
      await tx.$queryRaw(Prisma.sql`
        SELECT 1::int AS locked
        FROM pg_advisory_xact_lock(
          hashtextextended(${`translation-user:${BACKGROUND_TRANSLATION_IDENTITY}`}, 0)
        )
      `);
      const resetAt = new Date();
      const windowCutoff = new Date(resetAt.getTime() - TRANSLATION_LIMIT_WINDOW_MS);
      const policy = await getTranslationQuotaPolicy(tx);
      const cutoff =
        policy.budgetResetAt && policy.budgetResetAt > windowCutoff
          ? policy.budgetResetAt
          : windowCutoff;
      const count = await tx.translation_generation_attempt.count({
        where: {
          user_email: BACKGROUND_TRANSLATION_IDENTITY,
          created_at: { gte: cutoff },
        },
      });
      await tx.$executeRaw(Prisma.sql`
        UPDATE translation_runtime_setting
        SET budget_reset_at = ${resetAt},
            updated_at = ${resetAt}
        WHERE id = 'automatic'
      `);
      await tx.$executeRaw(Prisma.sql`
        INSERT INTO translation_admin_event (id, action, job_id, numeric_value)
        VALUES (${uuidv4()}, 'quota_budget_reset', NULL, ${count})
      `);
      return count;
    });
    return { resetAttempts };
  }

  async removeJob(id: string): Promise<{ removed: boolean; reason?: "missing" | "processing" }> {
    const job = await prisma.translation_job.findFirst({
      where: { id, requested_by: BACKGROUND_TRANSLATION_IDENTITY },
      select: { id: true, status: true },
    });
    if (!job) return { removed: false, reason: "missing" };
    if (job.status === "processing") return { removed: false, reason: "processing" };
    if (job.status !== "pending") return { removed: false, reason: "missing" };

    return prisma.$transaction(async (tx) => {
      const deleted = await tx.translation_job.deleteMany({
        where: {
          id,
          requested_by: BACKGROUND_TRANSLATION_IDENTITY,
          status: "pending",
        },
      });
      if (!deleted.count) return { removed: false, reason: "missing" as const };
      await tx.$executeRaw(Prisma.sql`
        INSERT INTO translation_admin_event (id, action, job_id, numeric_value)
        VALUES (${uuidv4()}, 'translation_job_removed', ${id}, NULL)
      `);
      return { removed: true };
    });
  }
}
