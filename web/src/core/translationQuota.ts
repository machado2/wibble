import { Prisma } from "@prisma/client";

export const TRANSLATION_LIMIT_WINDOW_MS = 60 * 60 * 1000;
export const DEFAULT_TRANSLATION_HOURLY_LIMIT = 10;
export const MAX_TRANSLATION_HOURLY_LIMIT = 100;
export const BACKGROUND_TRANSLATION_IDENTITY =
  "background-translations@wibble.internal";

type RawQueryClient = {
  $queryRaw<T = unknown>(query: Prisma.Sql): Promise<T>;
};

export const assertTranslationHourlyLimit = (value: unknown): number => {
  if (
    !Number.isInteger(value) ||
    Number(value) < 0 ||
    Number(value) > MAX_TRANSLATION_HOURLY_LIMIT
  ) {
    throw new RangeError(
      `hourlyLimit must be an integer between 0 and ${MAX_TRANSLATION_HOURLY_LIMIT}`
    );
  }
  return Number(value);
};

export const availableTranslationQueueCapacity = (
  hourlyLimit: number,
  usedAttempts: number,
  activeJobs: number
): number => Math.max(0, hourlyLimit - usedAttempts - activeJobs);

export const nextTranslationSlotAt = (
  orderedAttempts: Date[],
  hourlyLimit: number,
  windowMs: number
): Date | null => {
  if (orderedAttempts.length === 0 || hourlyLimit <= 0) return null;
  const releaseIndex = Math.max(0, orderedAttempts.length - hourlyLimit);
  return new Date(orderedAttempts[releaseIndex].getTime() + windowMs);
};

export const getTranslationQuotaPolicy = async (
  client: RawQueryClient
): Promise<{ hourlyLimit: number; budgetResetAt: Date | null }> => {
  const rows = await client.$queryRaw<
    Array<{ hourly_limit: number; budget_reset_at: Date | null }>
  >(Prisma.sql`
    SELECT hourly_limit, budget_reset_at
    FROM translation_runtime_setting
    WHERE id = 'automatic'
  `);
  return {
    hourlyLimit: rows[0]?.hourly_limit ?? DEFAULT_TRANSLATION_HOURLY_LIMIT,
    budgetResetAt: rows[0]?.budget_reset_at ?? null,
  };
};

export const getTranslationHourlyLimit = async (client: RawQueryClient): Promise<number> =>
  (await getTranslationQuotaPolicy(client)).hourlyLimit;
