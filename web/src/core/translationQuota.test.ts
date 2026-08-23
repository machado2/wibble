import { availableTranslationQueueCapacity } from "./translationQuota";

describe("translation queue quota admission", () => {
  test("rejects new work when used attempts and active jobs consume the budget", () => {
    expect(availableTranslationQueueCapacity(10, 10, 0)).toBe(0);
    expect(availableTranslationQueueCapacity(10, 7, 2)).toBe(1);
    expect(availableTranslationQueueCapacity(10, 7, 5)).toBe(0);
    expect(availableTranslationQueueCapacity(0, 0, 0)).toBe(0);
  });
});
