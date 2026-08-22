import { createHmac, randomUUID } from "node:crypto";
import { getWibbleConfig } from "./config";
import logger from "./logger";
import { SleepCoolDown, SleepOnError, SleepOnIdle } from "./sleep";

const WORKER_PATH = "/api/internal/translation-worker";
const signWorkerRequest = (secret: string, timestamp: number, nonce: string): string =>
  createHmac("sha256", secret)
    .update(`POST:${WORKER_PATH}:${timestamp}:${nonce}`)
    .digest("hex");

type RequestDependencies = {
  fetcher?: typeof fetch;
  secret?: string;
  nowSeconds?: number;
  baseUrl?: string;
  nonce?: string;
  timeoutMs?: number;
};

export const requestTranslationWork = async (
  dependencies: RequestDependencies = {}
): Promise<boolean> => {
  const timestamp = dependencies.nowSeconds ?? Math.floor(Date.now() / 1000);
  const nonce = dependencies.nonce ?? randomUUID();
  const secret = dependencies.secret ?? getWibbleConfig().secrets.translation_worker_secret;
  const baseUrl = dependencies.baseUrl ?? process.env.WIBBLE_WEB_INTERNAL_URL ?? "http://web:18001";
  const response = await (dependencies.fetcher ?? fetch)(`${baseUrl}${WORKER_PATH}`, {
    method: "POST",
    headers: {
      "x-wibble-worker-timestamp": String(timestamp),
      "x-wibble-worker-nonce": nonce,
      "x-wibble-worker-signature": signWorkerRequest(secret, timestamp, nonce),
    },
    signal: AbortSignal.timeout(dependencies.timeoutMs ?? 10 * 60 * 1000),
  });
  if (!response.ok) {
    throw new Error(`Translation worker endpoint returned HTTP ${response.status}`);
  }
  const result = (await response.json()) as { processed?: boolean };
  return result.processed === true;
};

export const translationGenerationLoop = async () => {
  logger.info("Starting translation generation loop");
  for (;;) {
    try {
      const processed = await requestTranslationWork();
      if (processed) await SleepCoolDown();
      else await SleepOnIdle();
    } catch (error) {
      logger.error(`Translation generation loop failed: ${String(error)}`);
      await SleepOnError();
    }
  }
};
