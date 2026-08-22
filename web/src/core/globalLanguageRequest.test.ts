import { resolveRequestLanguage } from "./globalLanguageRequest";

describe("resolveRequestLanguage", () => {
  test("prefers an explicit canonical query language", () => {
    expect(resolveRequestLanguage("pt-br", "wibble_lang=es", "fr;q=1")).toEqual({ language: "pt-BR", redirect: false });
  });

  test("uses a persisted original choice instead of the browser header", () => {
    expect(resolveRequestLanguage(undefined, "wibble_lang=original", "pt-BR, en;q=0.8")).toEqual({ language: null, redirect: false });
  });

  test("uses weighted browser language and requests one canonical redirect", () => {
    expect(resolveRequestLanguage(undefined, undefined, "en;q=0.4, pt-BR;q=0.9, es;q=0.7")).toEqual({ language: "pt-BR", redirect: true });
  });
});
