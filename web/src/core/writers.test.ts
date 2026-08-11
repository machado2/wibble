import type { WriterConfig } from "../../../config-runtime";
import { eligibleWriters, selectWriter, WriterSelectionError } from "./writers";

const writers: WriterConfig[] = [
  {
    id: "public",
    nickname: "Public writer",
    slug: "public/model",
    provider: "openrouter",
    available: true,
    admin_only: false,
  },
  {
    id: "admin",
    nickname: "Admin writer",
    slug: "admin-model",
    provider: "openai",
    available: true,
    admin_only: true,
  },
  {
    id: "off",
    nickname: "Disabled writer",
    slug: "disabled-model",
    provider: "openai",
    available: false,
    admin_only: false,
  },
];

describe("writer selection", () => {
  test("filters admin-only and unavailable writers", () => {
    expect(eligibleWriters(writers, false).map(({ id }) => id)).toEqual([
      "public",
    ]);
    expect(eligibleWriters(writers, true).map(({ id }) => id)).toEqual([
      "public",
      "admin",
    ]);
  });

  test("uses the requested eligible writer", () => {
    expect(selectWriter(writers, "admin", true).id).toBe("admin");
  });

  test("rejects an admin-only writer for a regular user", () => {
    expect(() => selectWriter(writers, "admin", false)).toThrow(
      WriterSelectionError
    );
  });

  test("randomizes only among eligible writers when omitted", () => {
    expect(selectWriter(writers, undefined, true, () => 0).id).toBe("public");
    expect(selectWriter(writers, undefined, true, () => 0.99).id).toBe("admin");
  });
});
