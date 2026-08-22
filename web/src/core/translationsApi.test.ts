jest.mock("./serverSession", () => ({
  getServerEmail: jest.fn().mockResolvedValue("reader@example.com"),
}));
jest.mock("./generationIdentity", () => ({
  requestIp: jest.fn().mockReturnValue("192.0.2.5"),
  generationSafetyIdentifier: jest.fn().mockReturnValue("safe-reader"),
}));

import handler from "../pages/api/translations/[slug]";
import {
  ArticleTranslationError,
  ArticleTranslationService,
} from "./ArticleTranslationService";
import { getServerEmail } from "./serverSession";

const mockGetServerEmail = jest.mocked(getServerEmail);

const response = () => {
  const res: any = {};
  res.status = jest.fn(() => res);
  res.json = jest.fn(() => res);
  res.setHeader = jest.fn(() => res);
  return res;
};

describe("article translations API", () => {
  beforeEach(() => {
    jest.clearAllMocks();
    mockGetServerEmail.mockResolvedValue("reader@example.com");
  });
  afterEach(() => jest.restoreAllMocks());

  test("generates a requested language view", async () => {
    jest
      .spyOn(ArticleTranslationService.prototype, "availableLanguages")
      .mockResolvedValue([]);
    jest.spyOn(ArticleTranslationService.prototype, "generate").mockResolvedValue({
      created: true,
      translation: {
        id: "translation-id",
        content_id: "article-id",
        language_code: "pt-BR",
        title: "Título",
        description: "Descrição",
        content: "Texto",
        model: "gpt-5.6-luna",
        created_at: new Date("2026-08-20T00:00:00Z"),
      },
    });
    const req: any = {
      method: "POST",
      query: { slug: "story" },
      body: { language: "pt-br" },
      headers: {},
      socket: {},
    };
    const res = response();

    await handler(req, res);

    expect(res.status).toHaveBeenCalledWith(201);
    expect(res.json).toHaveBeenCalledWith(
      expect.objectContaining({ languageCode: "pt-BR", created: true })
    );
    expect(ArticleTranslationService.prototype.generate).toHaveBeenCalledWith(
      "story",
      "pt-BR",
      "reader@example.com",
      "safe-reader"
    );
  });

  test("lists existing language views", async () => {
    jest
      .spyOn(ArticleTranslationService.prototype, "availableLanguages")
      .mockResolvedValue(["es", "pt-BR"]);
    const req: any = { method: "GET", query: { slug: "story" } };
    const res = response();

    await handler(req, res);

    expect(res.status).toHaveBeenCalledWith(200);
    expect(res.json).toHaveBeenCalledWith({ languages: ["es", "pt-BR"] });
  });

  test("requires authentication before paid generation", async () => {
    mockGetServerEmail.mockResolvedValue(null);
    const generate = jest.spyOn(ArticleTranslationService.prototype, "generate");
    const req: any = {
      method: "POST",
      query: { slug: "story" },
      body: { language: "pt-BR" },
    };
    const res = response();

    await handler(req, res);

    expect(res.status).toHaveBeenCalledWith(401);
    expect(generate).not.toHaveBeenCalled();
  });

  test("returns the durable generation limit with retry metadata", async () => {
    const generate = jest
      .spyOn(ArticleTranslationService.prototype, "generate")
      .mockRejectedValue(
        new ArticleTranslationError(
          "Translation generation limit reached",
          429,
          120
        )
      );
    const req: any = {
      method: "POST",
      query: { slug: "story" },
      body: { language: "pt-BR" },
      headers: {},
      socket: {},
    };
    const res = response();

    await handler(req, res);

    expect(res.status).toHaveBeenCalledWith(429);
    expect(res.setHeader).toHaveBeenCalledWith("Retry-After", "120");
    expect(generate).toHaveBeenCalled();
  });

  test("rejects unsupported language tags before generation", async () => {
    const generate = jest.spyOn(ArticleTranslationService.prototype, "generate");
    const req: any = {
      method: "POST",
      query: { slug: "story" },
      body: { language: "zz-ZZ" },
    };
    const res = response();

    await handler(req, res);

    expect(res.status).toHaveBeenCalledWith(400);
    expect(generate).not.toHaveBeenCalled();
  });

  test("rejects malformed language tags before generation", async () => {
    const generate = jest.spyOn(ArticleTranslationService.prototype, "generate");
    const req: any = {
      method: "POST",
      query: { slug: "story" },
      body: { language: "<script>" },
    };
    const res = response();

    await handler(req, res);

    expect(res.status).toHaveBeenCalledWith(400);
    expect(generate).not.toHaveBeenCalled();
  });
});
