/** @jest-environment jsdom */

import React from "react";
import { render, screen } from "@testing-library/react";
import { GlobalLanguageProvider } from "./GlobalLanguage";
import { NewsImageSelector } from "./NewsImageSelector";
import SearchBox from "./SearchBox";
import { TimeSelection } from "./TimeSelection";
import ArticleList from "./ArticleList";
import { VoteButtons } from "./VoteButtons";

const push = jest.fn();
const replace = jest.fn().mockResolvedValue(true);
const router = {
  pathname: "/",
  query: { lang: "pt-BR", sort: "most_voted" } as Record<string, string>,
  push,
  replace,
  isReady: true,
};

jest.mock("next/router", () => ({ useRouter: () => router }));
jest.mock("next-auth/react", () => ({
  useSession: () => ({ data: null }),
  signIn: jest.fn(),
}));
jest.mock("./NewsListItemUI", () => ({ NewsListItemUI: () => null }));

beforeAll(() => {
  Object.defineProperty(window, "matchMedia", {
    writable: true,
    value: jest.fn().mockImplementation(() => ({
      matches: false,
      addListener: jest.fn(),
      removeListener: jest.fn(),
      addEventListener: jest.fn(),
      removeEventListener: jest.fn(),
      dispatchEvent: jest.fn(),
    })),
  });
});

const renderLocalized = (ui: React.ReactNode) =>
  render(<GlobalLanguageProvider>{ui}</GlobalLanguageProvider>);

describe("localized public controls", () => {
  test("renders home tabs, search and period in Portuguese", () => {
    renderLocalized(
      <>
        <NewsImageSelector />
        <SearchBox />
        <TimeSelection />
      </>
    );

    expect(screen.getByRole("tab", { name: "Notícias" })).toBeTruthy();
    expect(screen.getByRole("tab", { name: "Imagens" })).toBeTruthy();
    expect(screen.getByPlaceholderText("Buscar")).toBeTruthy();
    expect(screen.getByText("Tudo")).toBeTruthy();
  });

  test("renders article empty state and vote labels in Portuguese", () => {
    renderLocalized(
      <>
        <ArticleList latestNews={[]} />
        <VoteButtons contentId="content-1" votes={0} />
      </>
    );

    expect(screen.getByText("Nenhum artigo encontrado.")).toBeTruthy();
    expect(screen.getByRole("button", { name: "Votar a favor" })).toBeTruthy();
    expect(screen.getByRole("button", { name: "Votar contra" })).toBeTruthy();
  });
});
