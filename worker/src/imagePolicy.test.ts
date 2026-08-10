import assert from "node:assert/strict";
import test from "node:test";
import {
  MAX_IMAGE_GENERATION_FAILURES,
  nextImageFailureState,
  reserveImageGenerationSlot,
} from "./imagePolicy";

test("an image is permanently failed on its third failure", () => {
  assert.deepEqual(nextImageFailureState(0), {
    failCount: 1,
    permanentlyFailed: false,
  });
  assert.deepEqual(nextImageFailureState(1), {
    failCount: 2,
    permanentlyFailed: false,
  });
  assert.deepEqual(nextImageFailureState(2), {
    failCount: MAX_IMAGE_GENERATION_FAILURES,
    permanentlyFailed: true,
  });
});

test("rate-limit reservations never overlap", () => {
  const first = reserveImageGenerationSlot(1_000, null, 30_000);
  assert.deepEqual(first, { runAtMs: 1_000, nextRunMs: 31_000 });

  const second = reserveImageGenerationSlot(2_000, first.nextRunMs, 30_000);
  assert.deepEqual(second, { runAtMs: 31_000, nextRunMs: 61_000 });
});
