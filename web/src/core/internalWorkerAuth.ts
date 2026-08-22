import { createHmac, timingSafeEqual } from "node:crypto";

const MAX_AGE_SECONDS = 60;
const NONCE_PATTERN = /^[a-f0-9-]{36}$/i;
const SIGNED_PATH = "/api/internal/translation-worker";

const payload = (timestamp: number, nonce: string) =>
  `POST:${SIGNED_PATH}:${timestamp}:${nonce}`;

export const signWorkerRequest = (secret: string, timestamp: number, nonce: string): string =>
  createHmac("sha256", secret).update(payload(timestamp, nonce)).digest("hex");

export const verifyWorkerRequest = (
  secret: string,
  timestampHeader: string | undefined,
  nonce: string | undefined,
  signature: string | undefined,
  nowSeconds = Math.floor(Date.now() / 1000)
): boolean => {
  if (!timestampHeader || !nonce || !signature || !NONCE_PATTERN.test(nonce)) return false;
  const timestamp = Number(timestampHeader);
  if (!Number.isInteger(timestamp) || Math.abs(nowSeconds - timestamp) > MAX_AGE_SECONDS) return false;
  const expected = Buffer.from(signWorkerRequest(secret, timestamp, nonce), "hex");
  const received = Buffer.from(signature, "hex");
  return expected.length === received.length && timingSafeEqual(expected, received);
};
