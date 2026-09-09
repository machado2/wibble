import assert from "node:assert/strict";
import test from "node:test";
import {
  generationErrorMessage,
  generationRetryDelaySeconds,
  shouldPermanentlyFailGeneration,
} from "./generationPolicy";

test("uses bounded exponential-style generation backoff", () => {
  assert.equal(generationRetryDelaySeconds(1), 60);
  assert.equal(generationRetryDelaySeconds(2), 120);
  assert.equal(generationRetryDelaySeconds(3), 300);
  assert.equal(generationRetryDelaySeconds(99), 1800);
});

test("fails permanently on permanent errors or the fifth attempt", () => {
  assert.equal(shouldPermanentlyFailGeneration(1, false), false);
  assert.equal(shouldPermanentlyFailGeneration(1, true), true);
  assert.equal(shouldPermanentlyFailGeneration(5, false), true);
});

test("bounds persisted error messages", () => {
  assert.equal(generationErrorMessage(new Error("broken")), "Error: broken");
  assert.equal(generationErrorMessage("x".repeat(600)).length, 500);
});
