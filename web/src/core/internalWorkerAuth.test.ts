import { signWorkerRequest, verifyWorkerRequest } from "./internalWorkerAuth";

describe("internal translation worker authentication", () => {
  test("accepts a fresh matching HMAC and rejects stale or changed signatures", () => {
    const now = 1_700_000_000;
    const nonce = "11111111-1111-4111-8111-111111111111";
    const signature = signWorkerRequest("secret-value", now, nonce);

    expect(verifyWorkerRequest("secret-value", String(now), nonce, signature, now)).toBe(true);
    expect(verifyWorkerRequest("secret-value", String(now - 61), nonce, signature, now)).toBe(false);
    expect(verifyWorkerRequest("secret-value", String(now), "22222222-2222-4222-8222-222222222222", signature, now)).toBe(false);
    expect(verifyWorkerRequest("different", String(now), nonce, signature, now)).toBe(false);
  });
});
