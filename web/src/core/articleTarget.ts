export type ArticleTarget = {
  url: string;
  slug: string;
};

export const normalizeArticleTarget = (
  value: string,
  siteUrl: string
): ArticleTarget => {
  const trimmed = value.trim();
  if (!trimmed) throw new Error("Missing target URL");

  const base = new URL(siteUrl);
  let target: URL;
  try {
    target = new URL(trimmed, base);
  } catch {
    throw new Error("Invalid target URL");
  }

  if (target.origin !== base.origin) {
    throw new Error("Target URL must belong to this site");
  }

  const match = target.pathname.match(/^\/content\/([^/]+)\/?$/);
  if (!match) {
    throw new Error("Target URL must use /content/article-slug");
  }

  let slug: string;
  try {
    slug = decodeURIComponent(match[1]).trim().toLowerCase();
  } catch {
    throw new Error("Invalid target URL encoding");
  }

  if (!slug || slug.length > 500 || slug.includes("/")) {
    throw new Error("Invalid article slug");
  }

  return {
    slug,
    url: `/content/${encodeURIComponent(slug)}`,
  };
};

export const articleTargetForSlug = (
  slug: string,
  siteUrl: string
): ArticleTarget =>
  normalizeArticleTarget(`/content/${encodeURIComponent(slug)}`, siteUrl);
