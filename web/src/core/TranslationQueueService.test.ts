import {
  BACKGROUND_TRANSLATION_IDENTITY,
  TranslationQueueService,
  type TranslationQueueJob,
} from "./TranslationQueueService";
import { ArticleTranslationError } from "./ArticleTranslationService";
import { isAutomaticTranslationLanguage } from "./translationLanguages";

const job: TranslationQueueJob = {
  id: "job-1",
  slug: "article-one",
  languageCode: "pt-BR",
  requestedBy: "USER@example.com",
  attempts: 0,
  leaseId: "lease-1",
};

const repository = () => ({
  enqueue: jest.fn().mockResolvedValue(2),
  claimNext: jest.fn().mockResolvedValue(job),
  complete: jest.fn().mockResolvedValue(undefined),
  retry: jest.fn().mockResolvedValue(undefined),
  rejectQuota: jest.fn().mockResolvedValue(undefined),
  fail: jest.fn().mockResolvedValue(undefined),
});

const translator = () => ({
  generate: jest.fn().mockResolvedValue({ created: true }),
});

describe("TranslationQueueService", () => {
  test("uses a distinct global identity for automatic background cost control", () => {
    expect(BACKGROUND_TRANSLATION_IDENTITY).toBe("background-translations@wibble.internal");
    expect(isAutomaticTranslationLanguage("pt-BR")).toBe(false);
    expect(isAutomaticTranslationLanguage("es")).toBe(false);
    expect(isAutomaticTranslationLanguage("nl")).toBe(false);
  });

  test("enqueues a bounded, canonicalized visible batch", async () => {
    const repo = repository();
    const service = new TranslationQueueService({ repository: repo, translator: translator() as any });

    const count = await service.enqueueVisible(
      ["one", "two", "two", ...Array.from({ length: 30 }, (_, index) => `article-${index}`)],
      "pt-br",
      "USER@example.com"
    );

    expect(count).toBe(2);
    expect(repo.enqueue).toHaveBeenCalledWith(
      expect.arrayContaining(["one", "two"]),
      "pt-BR",
      "user@example.com"
    );
    expect(repo.enqueue.mock.calls[0][0]).toHaveLength(20);
  });

  test("claims, translates and completes one durable job", async () => {
    const repo = repository();
    const generate = translator();
    const service = new TranslationQueueService({ repository: repo, translator: generate as any });

    expect(await service.processNext()).toEqual({ processed: true, status: "completed" });
    expect(generate.generate).toHaveBeenCalledWith(
      "article-one",
      "pt-BR",
      "user@example.com",
      expect.any(String)
    );
    expect(repo.complete).toHaveBeenCalledWith("job-1", "lease-1");
  });

  test("rejects quota exhaustion permanently instead of leaving work queued", async () => {
    const repo = repository();
    const generate = translator();
    generate.generate.mockRejectedValue(new ArticleTranslationError("limited", 429, 120));
    const service = new TranslationQueueService({ repository: repo, translator: generate as any });

    expect(await service.processNext()).toEqual({ processed: true, status: "rejected" });
    expect(repo.rejectQuota).toHaveBeenCalledWith(
      "job-1",
      "lease-1",
      "Quota de traduções esgotada"
    );
    expect(repo.retry).not.toHaveBeenCalled();
    expect(repo.fail).not.toHaveBeenCalled();
  });

  test("records a sanitized permanent failure after the final attempt", async () => {
    const repo = repository();
    repo.claimNext.mockResolvedValue({ ...job, attempts: 3 });
    const generate = translator();
    generate.generate.mockRejectedValue(new Error("provider secret details"));
    const service = new TranslationQueueService({ repository: repo, translator: generate as any });

    expect(await service.processNext()).toEqual({ processed: true, status: "failed" });
    expect(repo.fail).toHaveBeenCalledWith("job-1", "lease-1", "Falha ao gerar tradução");
  });
});
