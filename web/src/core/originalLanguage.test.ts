import {
  automaticTranslationTargets,
  detectOriginalLanguage,
  isSameAsOriginal,
} from "./originalLanguage";

describe("detectOriginalLanguage", () => {
  test("detects an English article", () => {
    expect(
      detectOriginalLanguage({
        title: "The big story",
        description: "A quick article and the trend",
        content: "This is the story and the news for the world.",
      })
    ).toBe("en");
  });

  test("detects a Brazilian Portuguese article", () => {
    expect(
      detectOriginalLanguage({
        title: "A grande notícia",
        description: "Uma história rápida",
        content: "Não é para ler com calma que você não vai entender tudo.",
      })
    ).toBe("pt-BR");
  });

  test("detects a non-latin (Ukrainian) original", () => {
    expect(
      detectOriginalLanguage({
        title: "Новини",
        description: "Історія",
        content: "Це український текст статті про новини.",
      })
    ).toBe("other");
  });

  test("falls back to other when the text is too ambiguous", () => {
    expect(
      detectOriginalLanguage({ title: "x", description: "y", content: "z" })
    ).toBe("other");
  });

  test("treats missing content without crashing", () => {
    expect(
      detectOriginalLanguage({ title: "The title", description: "some text the", content: null })
    ).toBe("en");
  });
});

describe("automaticTranslationTargets", () => {
  test("English original only targets Portuguese", () => {
    expect(automaticTranslationTargets("en")).toEqual(["pt-BR"]);
  });

  test("Portuguese original only targets English", () => {
    expect(automaticTranslationTargets("pt-BR")).toEqual(["en"]);
  });

  test("other originals target both", () => {
    expect(automaticTranslationTargets("other")).toEqual(["en", "pt-BR"]);
  });
});

describe("isSameAsOriginal", () => {
  test("matches the exact complementary-language check", () => {
    expect(isSameAsOriginal("en", "en")).toBe(true);
    expect(isSameAsOriginal("pt-BR", "pt-BR")).toBe(true);
    expect(isSameAsOriginal("pt", "pt-BR")).toBe(true);
    expect(isSameAsOriginal("pt-PT", "pt-BR")).toBe(true);
    expect(isSameAsOriginal("pt-BR", "en")).toBe(false);
    expect(isSameAsOriginal("es", "other")).toBe(false);
  });
});
