import { ArticleTranslationService } from "./ArticleTranslationService";

const article = {
  id: "article-id",
  slug: "story",
  title: "Original title",
  description: "Original description",
  content: "Original body",
  model: "gpt-5.6-luna",
  published: true,
  flagged: false,
};

const writer = {
  id: "luna",
  nickname: "Luna",
  slug: "gpt-5.6-luna",
  provider: "openai" as const,
  available: true,
  admin_only: false,
};

const translation = {
  id: "translation-id",
  content_id: article.id,
  language_code: "pt-BR",
  title: "Título",
  description: "Descrição",
  content: "Texto",
  model: writer.slug,
  created_at: new Date("2026-08-20T00:00:00Z"),
};

const setup = (existing: typeof translation | null = null) => {
  const repository = {
    getContent: jest.fn().mockResolvedValue(article),
    getTranslation: jest.fn().mockResolvedValue(existing),
    runTranslationGeneration: jest.fn(
      async (
        _contentId: string,
        _languageCode: string,
        _userEmail: string,
        _model: string,
        generate: () => Promise<{ title: string; description: string; content: string }>
      ) => {
        await generate();
        return { translation, created: true };
      }
    ),
    getTranslationLanguages: jest.fn().mockResolvedValue(["pt-BR"]),
  };
  const generator = {
    generateTranslation: jest.fn().mockResolvedValue({
      title: translation.title,
      description: translation.description,
      content: translation.content,
    }),
  };
  const service = new ArticleTranslationService({
    repository,
    generator,
    writers: [writer],
  });
  return { service, repository, generator };
};

describe("ArticleTranslationService", () => {
  test("returns a cached language view without generating it again", async () => {
    const { service, generator } = setup(translation);

    await expect(service.generate("story", "pt-br", "reader@example.com")).resolves.toEqual({
      translation,
      created: false,
    });
    expect(generator.generateTranslation).not.toHaveBeenCalled();
  });

  test("generates and stores a new language view on the same article", async () => {
    const { service, repository, generator } = setup();

    await expect(service.generate("story", "pt-br", "reader@example.com")).resolves.toEqual({
      translation,
      created: true,
    });
    expect(generator.generateTranslation).toHaveBeenCalledWith(
      writer,
      {
        title: article.title,
        description: article.description,
        content: article.content,
      },
      expect.stringContaining("Portuguese")
    );
    expect(repository.runTranslationGeneration).toHaveBeenCalledWith(
      article.id,
      "pt-BR",
      "reader@example.com",
      writer.slug,
      expect.any(Function)
    );
  });

  test("uses the database-coordinated result when another process created it", async () => {
    const { service, repository, generator } = setup();
    repository.runTranslationGeneration.mockResolvedValueOnce({
      translation,
      created: false,
    });

    await expect(
      service.generate("story", "pt-BR", "reader@example.com")
    ).resolves.toEqual({
      translation,
      created: false,
    });
    expect(generator.generateTranslation).not.toHaveBeenCalled();
  });

  test("coalesces simultaneous requests for the same article language", async () => {
    const { service, generator } = setup();

    const [first, second] = await Promise.all([
      service.generate("story", "pt-BR", "reader@example.com"),
      service.generate("story", "pt-BR", "reader@example.com"),
    ]);

    expect(first.translation).toEqual(translation);
    expect(second.translation).toEqual(translation);
    expect(generator.generateTranslation).toHaveBeenCalledTimes(1);
  });

  test("does not share a rate-limit failure between different users", async () => {
    const { service, repository } = setup();
    repository.runTranslationGeneration
      .mockRejectedValueOnce(new Error("first user blocked"))
      .mockResolvedValueOnce({ translation, created: false });

    const [first, second] = await Promise.allSettled([
      service.generate("story", "pt-BR", "blocked@example.com"),
      service.generate("story", "pt-BR", "eligible@example.com"),
    ]);

    expect(first.status).toBe("rejected");
    expect(second).toEqual({
      status: "fulfilled",
      value: { translation, created: false },
    });
    expect(repository.runTranslationGeneration).toHaveBeenCalledTimes(2);
  });

  test("lists only cached language views for the article", async () => {
    const { service } = setup();

    await expect(service.availableLanguages("story")).resolves.toEqual(["pt-BR"]);
  });
});
