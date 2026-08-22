/** @jest-environment jsdom */

import React from "react";
import { render, waitFor } from "@testing-library/react";
import { GlobalLanguageProvider, globalCopyForLanguage, useGlobalLanguage } from "./GlobalLanguage";

const replace = jest.fn().mockResolvedValue(true);
const router = {
  pathname: "/",
  query: { search: "space", afterId: "page-2" } as Record<string, string>,
  replace,
  isReady: true,
};

jest.mock("next/router", () => ({ useRouter: () => router }));

const SelectedLanguage = () => {
  const { language } = useGlobalLanguage();
  return <span>{language ?? "original"}</span>;
};

describe("GlobalLanguageProvider browser default", () => {
  beforeEach(() => {
    replace.mockClear();
    window.localStorage.clear();
    document.cookie = "wibble_lang=; Max-Age=0; Path=/";
    router.query = { search: "space", afterId: "page-2" };
    Object.defineProperty(window.navigator, "languages", {
      configurable: true,
      value: ["pt-BR", "en-US"],
    });
  });

  test("selects the browser language, preserves filters and resets pagination", async () => {
    render(<GlobalLanguageProvider><SelectedLanguage /></GlobalLanguageProvider>);

    await waitFor(() => expect(replace).toHaveBeenCalledTimes(1));
    expect(replace).toHaveBeenCalledWith({
      pathname: "/",
      query: { search: "space", lang: "pt-BR" },
    }, undefined, { shallow: false });
    expect(window.localStorage.getItem("wibble-global-language-v1")).toBe("pt-BR");
    expect(document.cookie).toContain("wibble_lang=pt-BR");
  });
});

describe("global UI copy", () => {
  test.each(["es", "fr", "de", "it", "uk", "pl", "ja", "zh-CN", "ar", "hi"])(
    "localizes selectable language %s instead of falling back to English",
    (language) => {
      const copy = globalCopyForLanguage(language);
      expect(copy.chooseLanguage).not.toBe("Choose language");
      expect(copy.promptDetails).not.toBe("How this image was requested");
      expect(copy.siteDescription).not.toBe(globalCopyForLanguage("en").siteDescription);
    }
  );
});
