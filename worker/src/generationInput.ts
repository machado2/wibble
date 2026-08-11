export type GenerationInput = {
  suggestion: string;
  safetyIdentifier?: string;
};

const isSafetyIdentifier = (value: unknown): value is string =>
  typeof value === "string" && /^[a-f0-9]{64}$/.test(value);

export const parseGenerationInput = (stored: string): GenerationInput => {
  try {
    const parsed = JSON.parse(stored);
    if (
      parsed &&
      typeof parsed === "object" &&
      typeof parsed.suggestion === "string"
    ) {
      return {
        suggestion: parsed.suggestion,
        safetyIdentifier: isSafetyIdentifier(parsed.safetyIdentifier)
          ? parsed.safetyIdentifier
          : undefined,
      };
    }
  } catch {
    // Older rows stored the suggestion directly.
  }

  return { suggestion: stored };
};
