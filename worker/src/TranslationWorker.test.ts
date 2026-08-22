import assert from "node:assert/strict";
import test from "node:test";
import { requestTranslationWork } from "./TranslationWorker";

test("requestTranslationWork signs the internal request and reports whether work was processed", async () => {
  let request: { url?: string; init?: RequestInit } = {};
  const fakeFetch = async (url: string | URL | Request, init?: RequestInit) => {
    request = { url: String(url), init };
    return new Response(JSON.stringify({ processed: true, status: "completed" }), {
      status: 200,
      headers: { "content-type": "application/json" },
    });
  };

  const processed = await requestTranslationWork({
    fetcher: fakeFetch as typeof fetch,
    secret: "worker-secret",
    nowSeconds: 1_700_000_000,
    baseUrl: "http://web:18001",
  });

  assert.equal(processed, true);
  assert.equal(request.url, "http://web:18001/api/internal/translation-worker");
  assert.equal(request.init?.method, "POST");
  assert.equal((request.init?.headers as Record<string, string>)["x-wibble-worker-timestamp"], "1700000000");
  assert.match(
    (request.init?.headers as Record<string, string>)["x-wibble-worker-nonce"],
    /^[a-f0-9-]{36}$/
  );
  assert.match(
    (request.init?.headers as Record<string, string>)["x-wibble-worker-signature"],
    /^[a-f0-9]{64}$/
  );
  assert.ok(request.init?.signal);
});
