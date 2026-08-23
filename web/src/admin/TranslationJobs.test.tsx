/** @jest-environment jsdom */

import React from "react";
import { act, fireEvent, render, screen, waitFor } from "@testing-library/react";
import { TranslationJobCard, TranslationQueueControls } from "./TranslationJobs";

const job = {
  id: "job-1",
  content_id: "article-123456789",
  language_code: "pt-BR",
  status: "failed",
  attempts: 0,
  lease_id: "12345678-abcd-4000-9000-123456789abc",
  created_at: "2026-08-22T11:00:00.000Z",
  updated_at: "2026-08-22T11:05:00.000Z",
  next_attempt_at: "2026-08-22T12:10:00.000Z",
  started_at: null,
  completed_at: "2026-08-22T11:05:00.000Z",
  last_error: "Quota de traduções esgotada",
};

test("renders quota rejection as terminal history on mobile", () => {
  render(<TranslationJobCard record={job} />);

  expect(screen.getByText("Falhou")).toBeTruthy();
  expect(screen.queryByText("Aguardando quota")).toBeNull();
  expect(screen.getByText("PT-BR")).toBeTruthy();
  expect(screen.getByText("0")).toBeTruthy();
  expect(screen.getByText("12345678…")).toBeTruthy();
  expect(screen.getByText("Quota de traduções esgotada")).toBeTruthy();
  expect(screen.getByText("Próxima tentativa")).toBeTruthy();
  expect(screen.getByText("Atualizada")).toBeTruthy();
});

test("shows an ordered operational queue and removes a pending task in one click", async () => {
  const fetchMock = jest.fn()
    .mockResolvedValueOnce({
      ok: true,
      json: async () => ({
        hourlyLimit: 10,
        used: 4,
        remaining: 6,
        windowMinutes: 60,
        nextReleaseAt: null,
        jobs: [
          {
            id: "active-job",
            contentId: "article-active",
            slug: "active-story",
            title: "Active story",
            languageCode: "es",
            status: "processing",
            attempts: 1,
            createdAt: "2026-08-22T10:00:00.000Z",
            nextAttemptAt: "2026-08-22T10:00:00.000Z",
            startedAt: "2026-08-22T10:01:00.000Z",
            lastError: null,
          },
          {
            id: "pending-job",
            contentId: "article-pending",
            slug: "pending-story",
            title: "Pending story",
            languageCode: "zh-CN",
            status: "pending",
            attempts: 0,
            createdAt: "2026-08-22T11:00:00.000Z",
            nextAttemptAt: "2026-08-22T12:00:00.000Z",
            startedAt: null,
            lastError: null,
          },
        ],
      }),
    })
    .mockResolvedValueOnce({ ok: true, json: async () => ({ removed: true }) })
    .mockResolvedValueOnce({
      ok: true,
      json: async () => ({ hourlyLimit: 10, used: 4, remaining: 6, windowMinutes: 60, jobs: [] }),
    });
  (global as any).fetch = fetchMock;

  render(<TranslationQueueControls />);

  expect(await screen.findByText("#1")).toBeTruthy();
  expect(screen.getByText("Active story")).toBeTruthy();
  expect(screen.getByText("Pending story")).toBeTruthy();
  expect(screen.getByRole("button", { name: "Em execução" }).hasAttribute("disabled")).toBe(true);

  fireEvent.click(screen.getByRole("button", { name: "Remover Pending story da fila" }));
  await waitFor(() =>
    expect(fetchMock).toHaveBeenCalledWith(
      "/api/admin/translation-operations?id=pending-job",
      expect.objectContaining({ method: "DELETE" })
    )
  );
});

test("changes the durable hourly quota from the queue controls", async () => {
  const state = {
    hourlyLimit: 10,
    used: 4,
    remaining: 6,
    windowMinutes: 60,
    nextReleaseAt: null,
    jobs: [],
  };
  const fetchMock = jest.fn()
    .mockResolvedValueOnce({ ok: true, json: async () => state })
    .mockResolvedValueOnce({ ok: true, json: async () => ({ hourlyLimit: 0 }) })
    .mockResolvedValueOnce({
      ok: true,
      json: async () => ({ ...state, hourlyLimit: 0, remaining: 0 }),
    });
  (global as any).fetch = fetchMock;

  render(<TranslationQueueControls />);
  const input = (await screen.findByLabelText("Gerações por hora")) as HTMLInputElement;
  fireEvent.change(input, { target: { value: "0" } });
  fireEvent.click(screen.getByRole("button", { name: "Salvar quota" }));

  await waitFor(() =>
    expect(fetchMock).toHaveBeenCalledWith(
      "/api/admin/translation-operations",
      expect.objectContaining({
        method: "PATCH",
        body: JSON.stringify({ hourlyLimit: 0 }),
      })
    )
  );
  expect(await screen.findByText("Geração automática pausada.")).toBeTruthy();
});

test("polling does not overwrite a quota value the administrator is editing", async () => {
  jest.useFakeTimers();
  let resolvePoll!: (value: unknown) => void;
  const state = { hourlyLimit: 10, used: 0, remaining: 10, windowMinutes: 60, jobs: [] };
  const fetchMock = jest.fn()
    .mockResolvedValueOnce({ ok: true, json: async () => state })
    .mockImplementationOnce(() => new Promise((resolve) => { resolvePoll = resolve; }));
  (global as any).fetch = fetchMock;

  render(<TranslationQueueControls />);
  const input = (await screen.findByLabelText("Gerações por hora")) as HTMLInputElement;
  await act(async () => { jest.advanceTimersByTime(15_000); });
  await waitFor(() => expect(fetchMock).toHaveBeenCalledTimes(2));
  fireEvent.change(input, { target: { value: "6" } });
  await act(async () => {
    resolvePoll({ ok: true, json: async () => state });
    await Promise.resolve();
  });

  expect(input.value).toBe("6");
  jest.useRealTimers();
});
