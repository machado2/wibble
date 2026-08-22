jest.mock("next-mdx-remote/serialize", () => ({
  serialize: jest.fn(async (markdown: string) => ({
    compiledSource: markdown,
    scope: {},
    frontmatter: {
      title: markdown.match(/title: '([^']*)'/)?.[1]?.replace(/''/g, "'") ?? "",
      description:
        markdown.match(/description: '([^']*)'/)?.[1]?.replace(/''/g, "'") ?? "",
    },
  })),
}));

import { ContentService } from "./ContentService";

const article = {
  id: "article-id",
  slug: "story",
  title: "Original title",
  description: "Original description",
  content: "Original body",
  image_id: null,
  created_at: new Date("2026-08-20T00:00:00Z"),
  votes: 4,
  votesRelation: [],
};

const translation = {
  title: "Título traduzido",
  description: "Descrição traduzida",
  content: "Texto traduzido",
  language_code: "pt-BR",
};

describe("ContentService translated article views", () => {
  test("serializes a cached translation as a view of the original article", async () => {
    const response = await new ContentService().parseResponse(
      article as any,
      translation as any,
      ["pt-BR", "es"]
    );

    expect(response.id).toBe(article.id);
    expect(response.content.frontmatter).toMatchObject({
      title: translation.title,
      description: translation.description,
    });
    expect(response.content.compiledSource).toContain("Texto traduzido");
    expect(response.languageCode).toBe("pt-BR");
    expect(response.availableLanguages).toEqual(["pt-BR", "es"]);
  });

  test("marks the original view while still exposing cached languages", async () => {
    const response = await new ContentService().parseResponse(
      article as any,
      null,
      ["pt-BR"]
    );

    expect(response.content.frontmatter.title).toBe(article.title);
    expect(response.languageCode).toBeNull();
    expect(response.availableLanguages).toEqual(["pt-BR"]);
  });
});
