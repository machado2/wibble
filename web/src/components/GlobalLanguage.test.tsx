/** @jest-environment jsdom */

import React from "react";
import { render, waitFor } from "@testing-library/react";
import { GlobalLanguageProvider, useGlobalLanguage } from "./GlobalLanguage";

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
