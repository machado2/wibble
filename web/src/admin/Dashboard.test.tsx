/** @jest-environment jsdom */

import React from "react";
import { render, screen, waitFor } from "@testing-library/react";
import { Dashboard } from "./Dashboard";

const overview = {
  totalArticles: 20,
  publishedArticles: 18,
  generatingArticles: 1,
  flaggedArticles: 1,
  unresolvedUrls: 0,
  searchesLast24Hours: 2,
  images: { completed: 5 },
  recentArticles: [],
  translations: { pending: 3, processing: 1, completed: 9, failed: 2, total: 11 },
  recentTranslationJobs: [
    {
      id: "job-1",
      slug: "article-one",
      title: "Article one",
      language_code: "pt-BR",
      status: "processing",
      attempts: 1,
      created_at: "2026-08-22T11:00:00.000Z",
      updated_at: "2026-08-22T11:01:00.000Z",
      last_error: null,
    },
  ],
};

beforeEach(() => {
  global.fetch = jest.fn().mockResolvedValue({
    ok: true,
    json: async () => overview,
  } as Response) as jest.Mock;
});

afterEach(() => {
  delete (global as { fetch?: typeof fetch }).fetch;
});

test("shows background translation progress and recent jobs", async () => {
  render(<Dashboard />);

  await waitFor(() => expect(screen.getByText("Traduções em background")).toBeTruthy());
  expect(screen.getByText("3 pendentes")).toBeTruthy();
  expect(screen.getByText("1 em processamento")).toBeTruthy();
  expect(screen.getByText("2 falhas")).toBeTruthy();
  expect(screen.getByText("Article one")).toBeTruthy();
  expect(screen.getByText("pt-BR")).toBeTruthy();
});
