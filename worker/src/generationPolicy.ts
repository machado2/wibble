export const MAX_GENERATION_ATTEMPTS = 5;
export const GENERATION_TIMEOUT_MS = 5 * 60 * 1000;
export const ABANDONED_PROCESSING_MINUTES = 10;

const RETRY_DELAYS_SECONDS = [60, 120, 300, 900, 1800] as const;

export const generationRetryDelaySeconds = (attempt: number): number =>
  RETRY_DELAYS_SECONDS[
    Math.min(Math.max(attempt, 1), RETRY_DELAYS_SECONDS.length) - 1
  ];

export const shouldPermanentlyFailGeneration = (
  attempt: number,
  permanent: boolean
): boolean => permanent || attempt >= MAX_GENERATION_ATTEMPTS;

export const generationErrorMessage = (error: unknown): string => {
  const message = error instanceof Error ? `${error.name}: ${error.message}` : String(error);
  return message.slice(0, 500);
};
