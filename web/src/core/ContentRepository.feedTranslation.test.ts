jest.mock("./PrismaWibble", () => ({ __esModule: true, default: {} }));

import { ContentRepository } from "./ContentRepository";

describe("ContentRepository translated home cards", () => {
  test("uses a matching persisted translation for the home title and description", () => {
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
    } as any);

    expect(item.title).toBe("Título traduzido");
    expect(item.description).toBe("Descrição traduzida");
  });
});
