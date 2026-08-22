/** @jest-environment jsdom */

import React from "react";
import { render, screen } from "@testing-library/react";
import { TranslationJobCard } from "./TranslationJobs";

const job = {
  id: "job-1",
  content_id: "article-123456789",
  language_code: "pt-BR",
  status: "pending",
  attempts: 0,
  lease_id: "12345678-abcd-4000-9000-123456789abc",
  created_at: "2026-08-22T11:00:00.000Z",
  updated_at: "2026-08-22T11:05:00.000Z",
  next_attempt_at: "2026-08-22T12:10:00.000Z",
  started_at: null,
  completed_at: null,
  last_error: "Limite temporário de traduções",
};

test("renders quota waits, attempts, lease and operational timestamps on mobile", () => {
  render(<TranslationJobCard record={job} />);

  expect(screen.getByText("Aguardando quota")).toBeTruthy();
  expect(screen.getByText("PT-BR")).toBeTruthy();
  expect(screen.getByText("0")).toBeTruthy();
  expect(screen.getByText("12345678…")).toBeTruthy();
  expect(screen.getByText("Limite temporário de traduções")).toBeTruthy();
  expect(screen.getByText("Próxima tentativa")).toBeTruthy();
  expect(screen.getByText("Atualizada")).toBeTruthy();
});
