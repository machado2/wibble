/** @jest-environment jsdom */

import "@testing-library/jest-dom";
import React from "react";
import { render, screen } from "@testing-library/react";
import Layout from "./Layout";

jest.mock("next-auth/react", () => ({
  useSession: () => ({ data: null, status: "unauthenticated" }),
  signIn: jest.fn(),
  signOut: jest.fn(),
}));
jest.mock("./useRuntimeConfig", () => ({
  useRuntimeConfig: () => ({ discordUrl: "https://discord.example", ssoUrl: "https://sso.example" }),
}));
jest.mock("next/router", () => ({
  useRouter: () => ({ pathname: "/", query: {}, replace: jest.fn(), isReady: true }),
}));

describe("Layout global language control", () => {
  beforeAll(() => {
    Object.defineProperty(window, "matchMedia", {
      configurable: true,
      value: () => ({
        matches: false,
        addListener: jest.fn(),
        removeListener: jest.fn(),
        addEventListener: jest.fn(),
        removeEventListener: jest.fn(),
      }),
    });
  });

  test("shows a discreet global language button beside the site title", () => {
    render(<Layout><main>Content</main></Layout>);

    const title = screen.getByRole("heading", { name: "The Wibble" });
    const language = screen.getByRole("button", { name: /language/i });

    expect(title.parentElement?.parentElement?.contains(language)).toBe(true);
    expect(language.getAttribute("title")).toBeTruthy();
  });
});
