/** @jest-environment jsdom */

import React from "react";
import { render, waitFor } from "@testing-library/react";
import { TransparentArticleTranslation } from "./TransparentArticleTranslation";

jest.mock("next-auth/react", () => ({
  useSession: () => ({ status: "authenticated", data: { user: { email: "reader@example.com" } } }),
}));

describe("TransparentArticleTranslation", () => {
  beforeEach(() => {
    jest.clearAllMocks();
    global.fetch = jest.fn().mockResolvedValue({ ok: true, json: async () => ({ languageCode: "pt-BR" }) } as Response);
  });

  test("generates one missing selected translation and reloads the article", async () => {
    const onReady = jest.fn().mockResolvedValue(undefined);
    const view = (
      <TransparentArticleTranslation
        slug="story"
        requestedLanguage="pt-BR"
        currentLanguage={null}
        availableLanguages={[]}
        onReady={onReady}
      />
    );
    const { rerender } = render(view);
    rerender(view);

    await waitFor(() => expect(onReady).toHaveBeenCalledTimes(1));
    expect(global.fetch).toHaveBeenCalledTimes(1);
    expect(global.fetch).toHaveBeenCalledWith("/api/translations/story", expect.objectContaining({
      method: "POST",
      body: JSON.stringify({ language: "pt-BR" }),
    }));
  });

  test("does not call generation when the selected translation is already loaded", () => {
    render(
      <TransparentArticleTranslation
        slug="story"
        requestedLanguage="pt-BR"
        currentLanguage="pt-BR"
        availableLanguages={["pt-BR"]}
        onReady={jest.fn()}
      />
    );
    expect(global.fetch).not.toHaveBeenCalled();
  });
});
