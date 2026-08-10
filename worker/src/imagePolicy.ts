export const MAX_IMAGE_GENERATION_FAILURES = 3;

export const nextImageFailureState = (currentFailCount: number) => {
  const failCount = currentFailCount + 1;
  return {
    failCount,
    permanentlyFailed: failCount >= MAX_IMAGE_GENERATION_FAILURES,
  };
};

export const reserveImageGenerationSlot = (
  nowMs: number,
  currentNextRunMs: number | null,
  intervalMs: number
) => {
  const runAtMs = Math.max(nowMs, currentNextRunMs ?? nowMs);
  return { runAtMs, nextRunMs: runAtMs + intervalMs };
};
