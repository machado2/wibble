export type TranslationStatusCounts = {
  pending: number;
  processing: number;
  completed: number;
  failed: number;
};

export type TranslationLanguageOperations = TranslationStatusCounts & {
  languageCode: string;
  translated: number;
  waitingForQuota: number;
};

type TranslationOperationsInput = {
  now: Date;
  limit: number;
  windowMs: number;
  eligibleArticles: number;
  attempts: Date[];
  statusCounts: TranslationStatusCounts;
  waitingForQuota: number;
  oldestPendingAt: Date | null;
  nextAttemptAt: Date | null;
  completedLastHour: number;
  completedLast24Hours: number;
  averageDurationSeconds: number | null;
  languages: TranslationLanguageOperations[];
};

const iso = (value: Date | null) => value?.toISOString() ?? null;
const percent = (value: number, total: number) =>
  total > 0 ? Math.round((value / total) * 100) : 0;

export const buildTranslationOperations = (input: TranslationOperationsInput) => {
  const attempts = input.attempts
    .filter((attempt) => attempt.getTime() >= input.now.getTime() - input.windowMs)
    .sort((left, right) => left.getTime() - right.getTime());
  const used = Math.min(attempts.length, input.limit);
  const remaining = Math.max(0, input.limit - used);
  const nextSlotAt = used > 0
    ? new Date(attempts[0].getTime() + input.windowMs)
    : null;
  const total = Object.values(input.statusCounts).reduce((sum, count) => sum + count, 0);
  const settled = input.statusCounts.completed + input.statusCounts.failed;
  const activeLanguages = input.languages.length;
  const translated = input.languages.reduce((sum, language) => sum + language.translated, 0);
  const possible = input.eligibleArticles * activeLanguages;

  return {
    updatedAt: input.now.toISOString(),
    cost: {
      limit: input.limit,
      used,
      remaining,
      windowMinutes: Math.round(input.windowMs / 60_000),
      nextSlotAt: iso(nextSlotAt),
      exhausted: remaining === 0,
    },
    queue: {
      ...input.statusCounts,
      total,
      settled,
      progressPercent: percent(settled, total),
      waitingForQuota: input.waitingForQuota,
      oldestPendingAt: iso(input.oldestPendingAt),
      nextAttemptAt: iso(input.nextAttemptAt),
    },
    throughput: {
      completedLastHour: input.completedLastHour,
      completedLast24Hours: input.completedLast24Hours,
      averageDurationSeconds: input.averageDurationSeconds,
    },
    coverage: {
      eligibleArticles: input.eligibleArticles,
      activeLanguages,
      translated,
      possible,
      percent: percent(translated, possible),
      languages: input.languages.map((language) => ({
        ...language,
        percent: percent(language.translated, input.eligibleArticles),
      })),
    },
  };
};

export type TranslationOperations = ReturnType<typeof buildTranslationOperations>;
