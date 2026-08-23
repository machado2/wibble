export type TranslationLanguage = {
  code: string;
  label: string;
  preferred: boolean;
};

const SUPPORTED_BASE_LANGUAGE_CODES = new Set(
  "aa ab ae af ak am an ar as av ay az ba be bg bh bi bm bn bo br bs ca ce ch co cr cs cu cv cy da de dv dz ee el en eo es et eu fa ff fi fj fo fr fy ga gd gl gn gu gv ha he hi ho hr ht hu hy hz ia id ie ig ii ik io is it iu ja jv ka kg ki kj kk kl km kn ko kr ks ku kv kw ky la lb lg li ln lo lt lu lv mg mh mi mk ml mn mr ms mt my na nb nd ne ng nl nn no nr nv ny oc oj om or os pa pi pl ps pt qu rm rn ro ru rw sa sc sd se sg si sk sl sm sn so sq sr ss st su sv sw ta te tg th ti tk tl tn to tr ts tt tw ty ug uk ur uz ve vi vo wa wo xh yi yo za zh zu".split(
    " "
  )
);

const POPULAR_LANGUAGE_CODES = [
  "en",
  "pt-BR",
  "es",
  "fr",
  "de",
  "it",
  "uk",
  "pl",
  "ja",
  "zh-CN",
  "ar",
  "hi",
] as const;

// The public catalog is not an authorization list for paid background work.
// Automatic languages remain empty until an explicit product policy is configured.
const AUTOMATIC_LANGUAGE_CODES = new Set<string>();

export const isAutomaticTranslationLanguage = (code: string): boolean => {
  try {
    return AUTOMATIC_LANGUAGE_CODES.has(resolveSupportedTranslationLanguage(code));
  } catch {
    return false;
  }
};

export const normalizeLanguageCode = (value: string): string => {
  const candidate = value.trim();
  if (
    !candidate ||
    candidate.length > 35 ||
    !/^[A-Za-z]{2,3}(?:-[A-Za-z0-9]{2,8})*$/.test(candidate)
  ) {
    throw new Error("Invalid language");
  }
  try {
    return new Intl.Locale(candidate).toString();
  } catch {
    throw new Error("Invalid language");
  }
};

export const resolveSupportedTranslationLanguage = (value: string): string => {
  const canonical = normalizeLanguageCode(value);
  const locale = new Intl.Locale(canonical);
  const language = locale.language;
  if (!SUPPORTED_BASE_LANGUAGE_CODES.has(language)) {
    throw new Error("Unsupported language");
  }
  if (language === "pt") {
    return locale.region === "PT" ? "pt-PT" : "pt-BR";
  }
  if (language === "zh") {
    return locale.script === "Hant" || ["TW", "HK", "MO"].includes(locale.region ?? "")
      ? "zh-TW"
      : "zh-CN";
  }
  return language;
};

export const languageLabel = (code: string, displayLocale: string): string => {
  let locale = "en";
  try {
    locale = normalizeLanguageCode(displayLocale);
  } catch {
    // English is a stable fallback for malformed browser locale data.
  }
  return new Intl.DisplayNames([locale], { type: "language" }).of(code) ?? code;
};

export const orderedTranslationLanguages = (
  browserLanguages: readonly string[],
  displayLocale: string
): TranslationLanguage[] => {
  const preferred = browserLanguages
    .map((language) => {
      try {
        return resolveSupportedTranslationLanguage(language);
      } catch {
        return null;
      }
    })
    .find((language): language is string => language !== null);
  const codes = [preferred, ...POPULAR_LANGUAGE_CODES].filter(
    (code): code is string => Boolean(code)
  );
  const seen = new Set<string>();
  return codes.flatMap((code) => {
    const canonical = normalizeLanguageCode(code);
    if (seen.has(canonical)) return [];
    seen.add(canonical);
    return [
      {
        code: canonical,
        label: languageLabel(canonical, displayLocale),
        preferred: canonical === preferred,
      },
    ];
  });
};
