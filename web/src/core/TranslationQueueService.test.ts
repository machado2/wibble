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
  getContentsBySlug: jest.fn().mockResolvedValue([]),
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
    expect(isAutomaticTranslationLanguage("pt-BR")).toBe(true);
    expect(isAutomaticTranslationLanguage("en")).toBe(true);
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

  test("enqueues only Portuguese for an English original", async () => {
    const repo = repository();
    repo.enqueue.mockResolvedValue(1);
    repo.getContentsBySlug.mockResolvedValue([
      { slug: "story", title: "The big news", description: "A story", content: "The quick and the article" },
    ]);
    const service = new TranslationQueueService({ repository: repo, translator: translator() as any });

    const count = await service.enqueueAutomatic(["STORY"]);
    expect(count).toBe(1);
    expect(repo.enqueue).toHaveBeenCalledTimes(1);
    expect(repo.enqueue).toHaveBeenCalledWith(["story"], "pt-BR", BACKGROUND_TRANSLATION_IDENTITY);
  });

  test("enqueues only English for a Brazilian Portuguese original", async () => {
    const repo = repository();
    repo.enqueue.mockResolvedValue(1);
    repo.getContentsBySlug.mockResolvedValue([
      { slug: "story", title: "A notícia", description: "Uma história", content: "Não para com que como também" },
    ]);
    const service = new TranslationQueueService({ repository: repo, translator: translator() as any });

    const count = await service.enqueueAutomatic(["story"]);
    expect(count).toBe(1);
    expect(repo.enqueue).toHaveBeenCalledWith(["story"], "en", BACKGROUND_TRANSLATION_IDENTITY);
  });

  test("enqueues both targets for a non-English/Portuguese original", async () => {
    const repo = repository();
    repo.enqueue.mockResolvedValue(1);
    repo.getContentsBySlug.mockResolvedValue([
      { slug: "story", title: "Новини", description: "Історія", content: "Це український текст статті" },
    ]);
    const service = new TranslationQueueService({ repository: repo, translator: translator() as any });

    const count = await service.enqueueAutomatic(["story"]);
    expect(count).toBe(2);
    expect(repo.enqueue).toHaveBeenCalledWith(["story"], "en", BACKGROUND_TRANSLATION_IDENTITY);
    expect(repo.enqueue).toHaveBeenCalledWith(["story"], "pt-BR", BACKGROUND_TRANSLATION_IDENTITY);
  });
});
