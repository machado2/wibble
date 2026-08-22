/** @jest-environment jsdom */

import "@testing-library/jest-dom";
import React from "react";
import { fireEvent, render, screen, waitFor } from "@testing-library/react";
import { ArticleTranslationControls } from "./ArticleTranslationControls";

const push = jest.fn().mockResolvedValue(true);
jest.mock("next/router", () => ({
  useRouter: () => ({
    pathname: "/content/[slug]",
    query: { slug: "story" },
    push,
  }),
}));

describe("ArticleTranslationControls", () => {
  beforeEach(() => {
    jest.clearAllMocks();
    Object.defineProperty(window.navigator, "languages", {
      configurable: true,
      value: ["pt-BR", "en-US"],
    });
    global.fetch = jest.fn().mockResolvedValue({
      ok: true,
      json: async () => ({ languageCode: "pt-BR", created: true }),
    } as Response);
  });

  test("shows the browser language as the first translation choice", async () => {
    render(
      <ArticleTranslationControls
        slug="story"
        currentLanguage={null}
        availableLanguages={[]}
      />
    );

    fireEvent.click(screen.getByRole("button", { name: /translate/i }));
    const choices = await screen.findAllByTestId("language-choice");

    expect(choices[0].textContent).toMatch(/português/i);
    expect(choices[0].textContent).toMatch(/browser/i);
  });

  test("generates a missing translation then opens that view", async () => {
    render(
      <ArticleTranslationControls
        slug="story"
        currentLanguage={null}
        availableLanguages={[]}
      />
    );

    fireEvent.click(screen.getByRole("button", { name: /translate/i }));
    fireEvent.click((await screen.findAllByTestId("language-choice"))[0]);

    await waitFor(() =>
      expect(global.fetch).toHaveBeenCalledWith(
        "/api/translations/story",
        expect.objectContaining({
          method: "POST",
          body: JSON.stringify({ language: "pt-BR" }),
        })
      )
    );
    await waitFor(() =>
      expect(push).toHaveBeenCalledWith({
        pathname: "/content/[slug]",
        query: { slug: "story", lang: "pt-BR" },
      })
    );
  });

  test("switches to an existing translation without generating it again", async () => {
    render(
      <ArticleTranslationControls
        slug="story"
        currentLanguage={null}
        availableLanguages={["pt-BR"]}
      />
    );

    fireEvent.click(screen.getByRole("button", { name: /translate/i }));
    fireEvent.click((await screen.findAllByTestId("language-choice"))[0]);

    await waitFor(() => expect(push).toHaveBeenCalled());
    expect(global.fetch).not.toHaveBeenCalled();
  });
});
