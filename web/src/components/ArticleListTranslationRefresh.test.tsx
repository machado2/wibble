/** @jest-environment jsdom */

import React from "react";
import { act, render, screen, waitFor } from "@testing-library/react";
import { loadNewsOnce } from "../core/loadNews";
import { NewsListItem } from "../core/NewsListItem";
import ArticleList from "./ArticleList";
import { GlobalLanguageProvider } from "./GlobalLanguage";

const router = {
  pathname: "/",
  query: { lang: "pt-BR" } as Record<string, string>,
  replace: jest.fn().mockResolvedValue(true),
  push: jest.fn().mockResolvedValue(true),
  isReady: true,
};

jest.mock("next/router", () => ({ useRouter: () => router }));
jest.mock("next/link", () => ({
  __esModule: true,
  default: ({ children, href, ...props }: any) => (
    <a href={typeof href === "string" ? href : "/content/story"} {...props}>
      {children}
    </a>
  ),
}));
jest.mock("../core/loadNews", () => ({
  loadNews: jest.fn(),
  loadNewsOnce: jest.fn(),
}));
jest.mock("next-auth/react", () => ({
  useSession: () => ({ data: null }),
  signIn: jest.fn(),
}));
jest.mock("./BoxedImage", () => ({ BoxedImage: () => null }));
jest.mock("./VoteButtons", () => ({ VoteButtons: () => null }));

const mockedLoadNewsOnce = loadNewsOnce as jest.MockedFunction<typeof loadNewsOnce>;

const article = (overrides: Partial<NewsListItem> = {}): NewsListItem => ({
  id: "article-1",
  slug: "story",
  title: "Original title",
  description: "Original description",
  imagePrompt: "",
  created_at: "2026-08-22T00:00:00.000Z",
  votes: 0,
  currentVote: 0,
  hotScore: 0,
  translationState: "pending",
  ...overrides,
});

describe("ArticleList background translation refresh", () => {
  beforeEach(() => {
    jest.useFakeTimers();
    mockedLoadNewsOnce.mockReset();
    Object.defineProperty(document, "hidden", {
      configurable: true,
      value: false,
    });
  });

  afterEach(() => {
    jest.useRealTimers();
  });

  test("keeps the original card visible and replaces it after translation completes", async () => {
    mockedLoadNewsOnce.mockResolvedValue([
      article({
        title: "Título traduzido",
        description: "Descrição traduzida",
        translationState: "translated",
      }),
    ]);

    render(
      <GlobalLanguageProvider>
        <ArticleList latestNews={[article()]} />
      </GlobalLanguageProvider>
    );

    expect(screen.getByRole("link", { name: "Original title" })).toBeTruthy();
    expect(screen.getByText("Traduzindo artigo…")).toBeTruthy();

    await act(async () => {
      jest.advanceTimersByTime(5000);
      await Promise.resolve();
    });

    await waitFor(() =>
      expect(screen.getByRole("link", { name: "Título traduzido" })).toBeTruthy()
    );
    expect(screen.queryByText("Traduzindo artigo…")).toBeNull();
    expect(mockedLoadNewsOnce).toHaveBeenCalledTimes(1);
  });

  test("preserves the current cards when a polling request fails", async () => {
    mockedLoadNewsOnce.mockRejectedValue(new Error("temporary outage"));
    render(
      <GlobalLanguageProvider>
        <ArticleList latestNews={[article()]} />
      </GlobalLanguageProvider>
    );

    await act(async () => {
      jest.advanceTimersByTime(5000);
      await Promise.resolve();
    });

    expect(screen.getByRole("link", { name: "Original title" })).toBeTruthy();
    expect(screen.getByText("Traduzindo artigo…")).toBeTruthy();
  });

  test("does not poll a terminally failed translation", () => {
    render(
      <GlobalLanguageProvider>
        <ArticleList latestNews={[article({ translationState: "failed" })]} />
      </GlobalLanguageProvider>
    );

    act(() => jest.advanceTimersByTime(60000));

    expect(screen.getByText("A tradução falhou.")).toBeTruthy();
    expect(mockedLoadNewsOnce).not.toHaveBeenCalled();
  });

  test("backs off and stops polling instead of querying forever", async () => {
    mockedLoadNewsOnce.mockResolvedValue([article()]);
    render(
      <GlobalLanguageProvider>
        <ArticleList latestNews={[article()]} />
      </GlobalLanguageProvider>
    );

    await act(async () => {
      await jest.advanceTimersByTimeAsync(10 * 60 * 1000);
    });

    expect(mockedLoadNewsOnce.mock.calls.length).toBeGreaterThan(1);
    expect(mockedLoadNewsOnce.mock.calls.length).toBeLessThanOrEqual(10);
  });

  test("pauses polling while the page is hidden", async () => {
    Object.defineProperty(document, "hidden", {
      configurable: true,
      value: true,
    });
    mockedLoadNewsOnce.mockResolvedValue([article()]);
    render(
      <GlobalLanguageProvider>
        <ArticleList latestNews={[article()]} />
      </GlobalLanguageProvider>
    );

    await act(async () => {
      await jest.advanceTimersByTimeAsync(60 * 1000);
    });

    expect(mockedLoadNewsOnce).not.toHaveBeenCalled();
  });
});
