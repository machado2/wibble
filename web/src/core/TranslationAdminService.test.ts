const findFirst = jest.fn();
const deleteMany = jest.fn();
const attemptCount = jest.fn();
const executeRaw = jest.fn();
const queryRaw = jest.fn();

const tx = {
  translation_job: { deleteMany },
  translation_generation_attempt: { count: attemptCount },
  $executeRaw: executeRaw,
  $queryRaw: queryRaw,
};

jest.mock("./PrismaWibble", () => ({
  __esModule: true,
  default: {
    translation_job: { findFirst: (...args: any[]) => findFirst(...args) },
    $transaction: (callback: any) => callback(tx),
  },
}));

import { TranslationAdminService } from "./TranslationAdminService";

const service = new TranslationAdminService();

describe("TranslationAdminService mutations", () => {
  beforeEach(() => {
    jest.clearAllMocks();
    deleteMany.mockResolvedValue({ count: 1 });
    executeRaw.mockResolvedValue(1);
    queryRaw.mockResolvedValue([]);
  });

  test.each(["completed", "failed"])("preserves %s history when called directly", async (status) => {
    findFirst.mockResolvedValue({ id: "job-terminal", status });

    await expect(service.removeJob("job-terminal")).resolves.toEqual({
      removed: false,
      reason: "missing",
    });
    expect(deleteMany).not.toHaveBeenCalled();
  });

  test("atomically deletes only a pending automatic job", async () => {
    findFirst.mockResolvedValue({ id: "job-pending", status: "pending" });

    await expect(service.removeJob("job-pending")).resolves.toEqual({ removed: true });
    expect(deleteMany).toHaveBeenCalledWith({
      where: {
        id: "job-pending",
        requested_by: "background-translations@wibble.internal",
        status: "pending",
      },
    });
  });

  test("counts only attempts since the previous reset", async () => {
    jest.useFakeTimers().setSystemTime(new Date("2026-08-22T18:30:00.500Z"));
    queryRaw
      .mockResolvedValueOnce([{ locked: 1 }])
      .mockResolvedValueOnce([{
        hourly_limit: 10,
        budget_reset_at: new Date("2026-08-22T18:15:00.250Z"),
      }]);
    attemptCount.mockResolvedValue(2);

    await expect(service.resetBudget()).resolves.toEqual({ resetAttempts: 2 });
    expect(attemptCount).toHaveBeenCalledWith({
      where: {
        user_email: "background-translations@wibble.internal",
        created_at: { gte: new Date("2026-08-22T18:15:00.250Z") },
      },
    });
    jest.useRealTimers();
  });
});
