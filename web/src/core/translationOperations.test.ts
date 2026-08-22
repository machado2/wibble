import { buildTranslationOperations } from "./translationOperations";

const now = new Date("2026-08-22T12:00:00.000Z");

test("summarizes the rolling automatic budget without exposing its identity", () => {
  const result = buildTranslationOperations({
    now,
    limit: 10,
    windowMs: 60 * 60 * 1000,
    eligibleArticles: 20,
    attempts: [
      new Date("2026-08-22T11:10:00.000Z"),
      new Date("2026-08-22T11:40:00.000Z"),
    ],
    statusCounts: { pending: 7, processing: 1, completed: 9, failed: 3 },
    waitingForQuota: 5,
    oldestPendingAt: new Date("2026-08-22T10:00:00.000Z"),
    nextAttemptAt: new Date("2026-08-22T12:05:00.000Z"),
    completedLastHour: 2,
    completedLast24Hours: 8,
    averageDurationSeconds: 42,
    languages: [],
  });

  expect(result.cost).toEqual({
    limit: 10,
    used: 2,
    remaining: 8,
    windowMinutes: 60,
    nextSlotAt: "2026-08-22T12:10:00.000Z",
    exhausted: false,
  });
  expect(JSON.stringify(result)).not.toContain("background-translations@");
});

test("calculates queue progress and honest per-language coverage", () => {
  const result = buildTranslationOperations({
    now,
    limit: 10,
    windowMs: 60 * 60 * 1000,
    eligibleArticles: 20,
    attempts: [],
    statusCounts: { pending: 7, processing: 1, completed: 9, failed: 3 },
    waitingForQuota: 5,
    oldestPendingAt: new Date("2026-08-22T10:00:00.000Z"),
    nextAttemptAt: new Date("2026-08-22T12:05:00.000Z"),
    completedLastHour: 2,
    completedLast24Hours: 8,
    averageDurationSeconds: 42,
    languages: [
      { languageCode: "pt-BR", translated: 15, pending: 1, processing: 1, completed: 14, failed: 4, waitingForQuota: 1 },
      { languageCode: "es", translated: 1, pending: 19, processing: 0, completed: 1, failed: 0, waitingForQuota: 19 },
      { languageCode: "zh-CN", translated: 0, pending: 20, processing: 0, completed: 0, failed: 0, waitingForQuota: 20 },
    ],
  });

  expect(result.queue).toMatchObject({
    total: 20,
    settled: 12,
    progressPercent: 60,
    waitingForQuota: 5,
    oldestPendingAt: "2026-08-22T10:00:00.000Z",
    nextAttemptAt: "2026-08-22T12:05:00.000Z",
  });
  expect(result.coverage).toMatchObject({
    eligibleArticles: 20,
    activeLanguages: 3,
    translated: 16,
    possible: 60,
    percent: 27,
  });
  expect(result.coverage.languages.map((language) => [language.languageCode, language.percent])).toEqual([
    ["pt-BR", 75],
    ["es", 5],
    ["zh-CN", 0],
  ]);
});
