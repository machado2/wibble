import { resolveSupportedTranslationLanguage } from "./translationLanguages";

type RequestLanguage = { language: string | null; redirect: boolean };

const canonicalOrNull = (value: string | undefined): string | null => {
  if (!value || value.length > 64) return null;
  try {
    return resolveSupportedTranslationLanguage(value);
  } catch {
    return null;
  }
};

const cookieLanguage = (cookieHeader: string | undefined): string | null | undefined => {
  if (!cookieHeader || cookieHeader.length > 16_384) return undefined;
  for (const part of cookieHeader.split(";")) {
    const [name, ...rawValue] = part.trim().split("=");
    if (name !== "wibble_lang") continue;
    try {
      const value = decodeURIComponent(rawValue.join("="));
      if (value === "original") return null;
      return canonicalOrNull(value) ?? undefined;
    } catch {
      return undefined;
    }
  }
  return undefined;
};

const acceptLanguage = (header: string | undefined): string | null => {
  if (!header || header.length > 8_192) return null;
  const candidates = header.split(",").slice(0, 30).flatMap((entry, index) => {
    const [tag, ...parameters] = entry.trim().split(";");
    if (tag === "*") return [];
    const qualityText = parameters.find((value) => value.trim().startsWith("q="));
    const quality = qualityText ? Number(qualityText.trim().slice(2)) : 1;
    const language = canonicalOrNull(tag);
    return language && Number.isFinite(quality) && quality > 0
      ? [{ language, quality: Math.min(1, quality), index }]
      : [];
  });
  candidates.sort((a, b) => b.quality - a.quality || a.index - b.index);
  return candidates[0]?.language ?? null;
};

export const resolveRequestLanguage = (
  queryLanguage: string | undefined,
  cookieHeader: string | undefined,
  acceptLanguageHeader: string | undefined
): RequestLanguage => {
  if (queryLanguage !== undefined) {
    return { language: canonicalOrNull(queryLanguage), redirect: false };
  }
  const cookie = cookieLanguage(cookieHeader);
  if (cookie !== undefined) {
    return { language: cookie, redirect: cookie !== null };
  }
  const browser = acceptLanguage(acceptLanguageHeader);
  return {
    language: browser === "en" ? null : browser,
    redirect: browser !== null && browser !== "en",
  };
};
