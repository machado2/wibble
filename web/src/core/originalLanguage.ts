export type OriginalLanguage = "en" | "pt-BR" | "other";

export const AUTOMATIC_TRANSLATION_TARGETS = ["en", "pt-BR"] as const;

type ArticleText = {
  title: string;
  description: string;
  content: string | null;
};

const countMatches = (text: string, patterns: RegExp[]): number =>
  patterns.reduce((total, pattern) => {
    pattern.lastIndex = 0;
    let count = 0;
    while (pattern.exec(text) !== null) count += 1;
    return total + count;
  }, 0);

const PT_PATTERNS = [
  /[ãõçâêôàáéíóúü]/gi,
  /\b(que|não|com|para|uma|você|voces?|são|foi|dos|das|ele|ela|eles|isso|esta|este|porque|também|muito|quando|como|mas|seu|sua|seus|suas|nosso|nossa|entre|sobre|depois|antes|onde|qual|quais)\b/gi,
];

const EN_PATTERNS = [
  /\b(the|and|is|are|with|you|that|this|for|not|have|has|was|were|will|would|from|they|them|their|there|what|which|about|into|than|then)\b/gi,
];

export const detectOriginalLanguage = (article: ArticleText): OriginalLanguage => {
  const text = `${article.title}\n${article.description}\n${article.content ?? ""}`.toLowerCase();
  if (!text.trim()) return "other";
  // Non-latin scripts (ukrainian/russian, japanese, chinese, arabic, hindi, ...):
  // never guess en/pt-BR from them, always translate to both.
  if (/[\u0400-\u04FF\u3040-\u30FF\u4E00-\u9FFF\u0600-\u06FF\u0900-\u097F]/.test(text)) {
    return "other";
  }
  const ptScore = countMatches(text, PT_PATTERNS);
  const enScore = countMatches(text, EN_PATTERNS);
  if (ptScore >= 2 && ptScore > enScore) return "pt-BR";
  if (enScore >= 2 && enScore > ptScore) return "en";
  return "other";
};

export const automaticTranslationTargets = (
  original: OriginalLanguage
): string[] => {
  if (original === "en") return ["pt-BR"];
  if (original === "pt-BR") return ["en"];
  return [...AUTOMATIC_TRANSLATION_TARGETS];
};

export const isSameAsOriginal = (
  targetLanguageCode: string,
  original: OriginalLanguage
): boolean => {
  const target = targetLanguageCode.trim().toLowerCase();
  if (original === "en") return target === "en";
  if (original === "pt-BR") return target === "pt-br" || target === "pt-pt" || target === "pt";
  return false;
};
