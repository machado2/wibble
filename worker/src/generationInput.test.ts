import assert from "node:assert/strict";
import test from "node:test";
import { parseGenerationInput } from "./generationInput";

test("parses a versioned generation input", () => {
  const safetyIdentifier = "a".repeat(64);
  assert.deepEqual(
    parseGenerationInput(
      JSON.stringify({ version: 1, suggestion: "A prompt", safetyIdentifier })
    ),
    { suggestion: "A prompt", safetyIdentifier }
  );
});

test("keeps compatibility with plain historical inputs", () => {
  assert.deepEqual(parseGenerationInput("A historical prompt"), {
    suggestion: "A historical prompt",
  });
});

test("does not trust an invalid stored identifier", () => {
  assert.deepEqual(
    parseGenerationInput(
      JSON.stringify({ suggestion: "A prompt", safetyIdentifier: "raw-ip" })
    ),
    { suggestion: "A prompt", safetyIdentifier: undefined }
  );
});
