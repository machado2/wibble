/** @jest-environment jsdom */

import React from "react";
import { act, render, screen, waitFor } from "@testing-library/react";
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
  translationOperations: {
    updatedAt: "2026-08-22T12:00:00.000Z",
    cost: {
      limit: 10,
      used: 10,
      remaining: 0,
      windowMinutes: 60,
      nextSlotAt: "2026-08-22T12:10:00.000Z",
      exhausted: true,
    },
    queue: {
      pending: 3,
      processing: 1,
      completed: 9,
      failed: 2,
      total: 15,
      settled: 11,
      progressPercent: 73,
      waitingForQuota: 3,
      oldestPendingAt: "2026-08-22T10:00:00.000Z",
      nextAttemptAt: "2026-08-22T12:05:00.000Z",
    },
    throughput: {
      completedLastHour: 4,
      completedLast24Hours: 9,
      averageDurationSeconds: 42,
    },
    coverage: {
      eligibleArticles: 18,
      activeLanguages: 2,
      translated: 11,
      possible: 36,
      percent: 31,
      languages: [
        {
          languageCode: "pt-BR",
          translated: 10,
          pending: 2,
          processing: 1,
          completed: 8,
          failed: 2,
          waitingForQuota: 2,
          percent: 56,
        },
        {
          languageCode: "es",
          translated: 1,
          pending: 1,
          processing: 0,
          completed: 1,
          failed: 0,
          waitingForQuota: 1,
          percent: 6,
        },
      ],
    },
  },
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
  jest.useRealTimers();
  jest.restoreAllMocks();
  delete (global as { fetch?: typeof fetch }).fetch;
});

test("shows background translation progress and recent jobs", async () => {
  render(<Dashboard />);

  await waitFor(() => expect(screen.getByText("Traduções em background")).toBeTruthy());
  expect(screen.getByText("3 pendentes")).toBeTruthy();
  expect(screen.getByText("1 em processamento")).toBeTruthy();
  expect(screen.getAllByText("2 falhas").length).toBeGreaterThan(0);
  expect(screen.getByText("Article one")).toBeTruthy();
  expect(screen.getByText("pt-BR")).toBeTruthy();
  expect(screen.getByText("10 de 10 usadas")).toBeTruthy();
  expect(screen.getByText("0 disponíveis agora")).toBeTruthy();
  expect(screen.getByText("3 esperando a quota")).toBeTruthy();
  expect(screen.getByText("73% dos jobs encerrados")).toBeTruthy();
  expect(screen.getByText("Cobertura por idioma")).toBeTruthy();
  expect(screen.getByText("56%")).toBeTruthy();
  expect(screen.getByText(/limite é global para toda a automação/i)).toBeTruthy();
});

test("does not overlap automatic dashboard refreshes", async () => {
  jest.useFakeTimers();
  const pendingRefresh = new Promise<void>(() => undefined);
  (global.fetch as jest.Mock)
    .mockResolvedValueOnce({ ok: true, json: async () => overview } as Response)
    .mockImplementationOnce(() => pendingRefresh);

  render(<Dashboard />);
  await waitFor(() => expect(screen.getByText("10 de 10 usadas")).toBeTruthy());
  await act(async () => {
    await jest.advanceTimersByTimeAsync(30_000);
  });

  expect(global.fetch).toHaveBeenCalledTimes(2);
});

test("does not start polling while the initial dashboard request is pending", async () => {
  jest.useFakeTimers();
  (global.fetch as jest.Mock).mockImplementation(() => new Promise<void>(() => undefined));

  render(<Dashboard />);
  await act(async () => {
    await jest.advanceTimersByTimeAsync(30_000);
  });

  expect(global.fetch).toHaveBeenCalledTimes(1);
});

test("keeps the last overview visible when an automatic refresh fails", async () => {
  jest.useFakeTimers();
  jest.spyOn(console, "error").mockImplementation(() => undefined);
  (global.fetch as jest.Mock)
    .mockResolvedValueOnce({ ok: true, json: async () => overview } as Response)
    .mockRejectedValueOnce(new Error("temporary outage"));

  render(<Dashboard />);
  await waitFor(() => expect(screen.getByText("10 de 10 usadas")).toBeTruthy());
  await act(async () => {
    await jest.advanceTimersByTimeAsync(15_000);
  });

  expect(screen.getByText("10 de 10 usadas")).toBeTruthy();
  expect(screen.getByText(/Não foi possível atualizar os indicadores agora\./)).toBeTruthy();
});
