jest.mock("./PrismaWibble", () => ({
  __esModule: true,
  default: {
    content_translation: {
      findUnique: jest.fn(),
      findMany: jest.fn(),
      create: jest.fn(),
    },
    translation_generation_attempt: {
      deleteMany: jest.fn(),
      create: jest.fn(),
    },
    $transaction: jest.fn(),
  },
}));

import prisma from "./PrismaWibble";
import { ContentRepository } from "./ContentRepository";

const translationModel = (prisma as any).content_translation;

describe("ContentRepository article translations", () => {
  beforeEach(() => jest.clearAllMocks());

  test("loads a cached language by article and canonical language code", async () => {
    const cached = { id: "translation-id", language_code: "pt-BR" };
    translationModel.findUnique.mockResolvedValue(cached);

    await expect(
      new ContentRepository().getTranslation("article-id", "pt-BR")
    ).resolves.toBe(cached);
    expect(translationModel.findUnique).toHaveBeenCalledWith({
      where: {
        content_id_language_code: {
          content_id: "article-id",
          language_code: "pt-BR",
        },
      },
    });
  });

  test("returns cached language codes in stable order", async () => {
    translationModel.findMany.mockResolvedValue([
      { language_code: "es" },
      { language_code: "pt-BR" },
    ]);

    await expect(
      new ContentRepository().getTranslationLanguages("article-id")
    ).resolves.toEqual(["es", "pt-BR"]);
  });

  test("serializes cache ownership and records one durable paid generation", async () => {
    const persisted = { id: "translation-id", language_code: "pt-BR" };
    const tx = {
      $queryRaw: jest.fn().mockResolvedValue([]),
      content_translation: {
        findUnique: jest.fn().mockResolvedValue(null),
        create: jest.fn().mockResolvedValue(persisted),
      },
      translation_generation_attempt: {
        findMany: jest.fn().mockResolvedValue([]),
        create: jest.fn().mockResolvedValue({ id: "attempt-id" }),
      },
    };
    (prisma as any).$transaction.mockImplementation((callback: any) => callback(tx));
    const generate = jest.fn().mockResolvedValue({
      title: "Título",
      description: "Descrição",
      content: "Texto",
    });

    await expect(
      new ContentRepository().runTranslationGeneration(
        "article-id",
        "pt-BR",
        "Reader@Example.com",
        "writer",
        generate
      )
    ).resolves.toEqual({ translation: persisted, created: true });

    expect(tx.$queryRaw).toHaveBeenCalledTimes(2);
    expect((prisma as any).translation_generation_attempt.create).toHaveBeenCalledWith({
      data: expect.objectContaining({
        user_email: "reader@example.com",
        content_id: "article-id",
        language_code: "pt-BR",
      }),
    });
    expect(generate).toHaveBeenCalledTimes(1);
  });

  test("keeps the paid attempt when generation fails", async () => {
    const tx = {
      $queryRaw: jest.fn().mockResolvedValue([]),
      content_translation: {
        findUnique: jest.fn().mockResolvedValue(null),
        create: jest.fn(),
      },
      translation_generation_attempt: {
        findMany: jest.fn().mockResolvedValue([]),
      },
    };
    (prisma as any).$transaction.mockImplementation((callback: any) => callback(tx));
    const generate = jest.fn().mockRejectedValue(new Error("provider failed"));

    await expect(
      new ContentRepository().runTranslationGeneration(
        "article-id",
        "pt-BR",
        "reader@example.com",
        "writer",
        generate
      )
    ).rejects.toThrow("provider failed");

    expect((prisma as any).translation_generation_attempt.create).toHaveBeenCalledTimes(1);
    expect(tx.content_translation.create).not.toHaveBeenCalled();
  });

  test("reuses a translation created while waiting without charging or generating", async () => {
    const persisted = { id: "translation-id", language_code: "pt-BR" };
    const tx = {
      $queryRaw: jest.fn().mockResolvedValue([]),
      content_translation: {
        findUnique: jest.fn().mockResolvedValue(persisted),
      },
      translation_generation_attempt: {
        findMany: jest.fn(),
        create: jest.fn(),
      },
    };
    (prisma as any).$transaction.mockImplementation((callback: any) => callback(tx));
    const generate = jest.fn();

    await expect(
      new ContentRepository().runTranslationGeneration(
        "article-id",
        "pt-BR",
        "reader@example.com",
        "writer",
        generate
      )
    ).resolves.toEqual({ translation: persisted, created: false });

    expect((prisma as any).translation_generation_attempt.create).not.toHaveBeenCalled();
    expect(generate).not.toHaveBeenCalled();
  });
});
