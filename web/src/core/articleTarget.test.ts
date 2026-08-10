import { normalizeArticleTarget } from "./articleTarget";

describe("normalizeArticleTarget", () => {
  test("accepts a content path and strips query parameters", () => {
    expect(normalizeArticleTarget("/content/Requested-Story?from=test")).toEqual({
      slug: "requested-story",
      url: "/content/requested-story",
    });
  });

  test("accepts an absolute URL on the configured site", () => {
    expect(
      normalizeArticleTarget("https://wibble.fbmac.net/content/a-new-story"),
    ).toEqual({ slug: "a-new-story", url: "/content/a-new-story" });
  });

  test.each([
    "https://example.com/content/story",
    "/admin",
    "/content/one/two",
    "",
  ])("rejects invalid or external targets: %s", (value) => {
    expect(() => normalizeArticleTarget(value)).toThrow();
  });
});
