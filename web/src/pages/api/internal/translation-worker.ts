import type { NextApiRequest, NextApiResponse } from "next";
import { getWibbleConfig } from "../../../../../config-runtime";
import prisma from "../../../core/PrismaWibble";
import { verifyWorkerRequest } from "../../../core/internalWorkerAuth";
import { TranslationQueueService } from "../../../core/TranslationQueueService";

const header = (value: string | string[] | undefined): string | undefined =>
  Array.isArray(value) ? value[0] : value;

export default async function handler(req: NextApiRequest, res: NextApiResponse) {
  if (req.method !== "POST") {
    res.status(405).json({ error: "Method not allowed" });
    return;
  }
  const config = getWibbleConfig();
  const timestamp = header(req.headers["x-wibble-worker-timestamp"]);
  const nonce = header(req.headers["x-wibble-worker-nonce"]);
  const signature = header(req.headers["x-wibble-worker-signature"]);
  if (!verifyWorkerRequest(config.secrets.translation_worker_secret, timestamp, nonce, signature)) {
    res.status(401).json({ error: "Unauthorized" });
    return;
  }

  const accepted = await prisma.$transaction(async (tx) => {
    await tx.$executeRaw`
      DELETE FROM translation_worker_request
      WHERE created_at < NOW() - INTERVAL '2 minutes'
    `;
    const inserted = await tx.$executeRaw`
      INSERT INTO translation_worker_request (nonce, created_at)
      VALUES (${nonce!}, NOW())
      ON CONFLICT (nonce) DO NOTHING
    `;
    return inserted === 1;
  });
  if (!accepted) {
    res.status(409).json({ error: "Request already processed" });
    return;
  }

  const result = await new TranslationQueueService().processNext();
  res.status(200).json(result);
}
