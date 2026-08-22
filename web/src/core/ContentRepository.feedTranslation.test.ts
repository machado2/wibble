jest.mock("./PrismaWibble", () => ({ __esModule: true, default: {} }));

import { ContentRepository } from "./ContentRepository";

describe("ContentRepository translated home cards", () => {
  test("marks a matching persisted translation for the home title and description", () => {
    const item = new ContentRepository().parseNewsListItem({
      id: "article-id",
      slug: "story",
      title: "Original title",
      description: "Original description",
      image_prompt: "A blue bird",
      created_at: new Date("2026-08-22T00:00:00Z"),
      votes: 7,
      hot_score: 2,
      translations: [{ title: "Título traduzido", description: "Descrição traduzida" }],
    } as any, true);

    expect(item.title).toBe("Título traduzido");
    expect(item.description).toBe("Descrição traduzida");
    expect(item.translationState).toBe("translated");
  });

  test("marks a requested automatic translation as pending while it is missing", () => {
    const item = new ContentRepository().parseNewsListItem({
      id: "article-id",
      slug: "story",
      title: "Original title",
      description: "Original description",
      image_prompt: "A blue bird",
      created_at: new Date("2026-08-22T00:00:00Z"),
      votes: 7,
      hot_score: 2,
      translations: [],
    } as any, true);

    expect(item.title).toBe("Original title");
    expect(item.translationState).toBe("pending");
  });

  test("marks a terminally failed translation job instead of polling forever", () => {
    const item = new ContentRepository().parseNewsListItem({
      id: "article-id",
      slug: "story",
      title: "Original title",
      description: "Original description",
      image_prompt: "A blue bird",
      created_at: new Date("2026-08-22T00:00:00Z"),
      votes: 7,
      hot_score: 2,
      translations: [],
      translationJobs: [{ status: "failed" }],
    } as any, true);

    expect(item.translationState).toBe("failed");
  });

  test("does not advertise background work when translation was not queued", () => {
    const item = new ContentRepository().parseNewsListItem({
      id: "article-id",
      slug: "story",
      title: "Original title",
      description: "Original description",
      image_prompt: "A blue bird",
      created_at: new Date("2026-08-22T00:00:00Z"),
      votes: 7,
      hot_score: 2,
      translations: [],
    } as any, false);

    expect(item.translationState).toBeUndefined();
  });
});
