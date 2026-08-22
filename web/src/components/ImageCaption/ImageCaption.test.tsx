/** @jest-environment jsdom */

import React from "react";
import { render, screen } from "@testing-library/react";
import { ImageCaption } from "./ImageCaption";
import { GlobalLanguageProvider, globalCopyForLanguage } from "../GlobalLanguage";

jest.mock("next/router", () => ({
  useRouter: () => ({ pathname: "/content/[slug]", query: { slug: "story", lang: "pt-BR" }, replace: jest.fn(), isReady: true }),
}));

class IntersectionObserverMock {
  observe() {}
  unobserve() {}
  disconnect() {}
}

beforeAll(() => {
  Object.defineProperty(window, "IntersectionObserver", { configurable: true, value: IntersectionObserverMock });
  Object.defineProperty(global, "IntersectionObserver", { configurable: true, value: IntersectionObserverMock });
});

describe("ImageCaption prompt explanation", () => {
  test("explains in the selected language what prompt and negative prompt mean", () => {
    render(
      <GlobalLanguageProvider>
        <ImageCaption imgid="image-id" alt="Uma cidade futurista" prompt="A futuristic city###No blur" />
      </GlobalLanguageProvider>
    );

    expect(screen.getByText(/instrução fornecida ao gerador de imagens/i)).toBeTruthy();
    expect(screen.getByText(/lista detalhes.*deve evitar/i)).toBeTruthy();
    expect(screen.getByText(/A futuristic city/)).toBeTruthy();
    expect(screen.getByText(/No blur/)).toBeTruthy();
  });

  test("uses the copy explicitly passed by a lightbox root outside the provider", () => {
    render(
      <ImageCaption
        imgid="image-id"
        alt="Uma cidade futurista"
        prompt="A futuristic city###No blur"
        copy={globalCopyForLanguage("pt-BR")}
      />
    );

    expect(screen.getByText("Como esta imagem foi solicitada")).toBeTruthy();
    expect(screen.getByText(/instrução fornecida ao gerador de imagens/i)).toBeTruthy();
  });
});
