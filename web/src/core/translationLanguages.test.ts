import {
  normalizeLanguageCode,
  orderedTranslationLanguages,
  resolveSupportedTranslationLanguage,
} from "./translationLanguages";

describe("translation language choices", () => {
  test("puts the browser requested language first and marks it as preferred", () => {
    const languages = orderedTranslationLanguages(["pt-BR", "en-US"], "pt-BR");

    expect(languages[0]).toMatchObject({
      code: "pt-BR",
      preferred: true,
    });
    expect(languages[0].label.toLocaleLowerCase("pt-BR")).toContain("português");
  });

  test("keeps an uncommon valid browser language available at the top", () => {
    const languages = orderedTranslationLanguages(["nl-NL"], "en");

    expect(languages[0]).toMatchObject({ code: "nl", preferred: true });
    expect(languages.some((language) => language.code === "pt-BR")).toBe(true);
  });

  test("maps browser variants into a finite supported catalog", () => {
    expect(resolveSupportedTranslationLanguage("en-US")).toBe("en");
    expect(resolveSupportedTranslationLanguage("nl-NL")).toBe("nl");
    expect(resolveSupportedTranslationLanguage("zh-TW")).toBe("zh-TW");
    expect(() => resolveSupportedTranslationLanguage("zz-ZZ")).toThrow(
      "Unsupported language"
    );
  });

  test("deduplicates canonical language tags", () => {
    const languages = orderedTranslationLanguages(["pt-br", "pt-BR"], "en");

    expect(languages.filter((language) => language.code === "pt-BR")).toHaveLength(1);
  });

  test.each(["", "not_a_language", "<script>", "x".repeat(36)])(
    "rejects invalid language tag %p",
    (language) => {
      expect(() => normalizeLanguageCode(language)).toThrow("Invalid language");
    }
  );
});
