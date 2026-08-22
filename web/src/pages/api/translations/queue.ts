import type { NextApiRequest, NextApiResponse } from "next";
import { getServerEmail } from "../../../core/serverSession";
import { TranslationQueueService } from "../../../core/TranslationQueueService";
import { resolveSupportedTranslationLanguage } from "../../../core/translationLanguages";

export default async function handler(req: NextApiRequest, res: NextApiResponse) {
  if (req.method !== "POST") {
    res.status(405).json({ error: "Method not allowed" });
    return;
  }
  const email = await getServerEmail(req, res);
  if (!email) {
    res.status(401).json({ error: "Sign in to queue translations" });
    return;
  }
  const language = typeof req.body?.language === "string" ? req.body.language : "";
  const slugs = Array.isArray(req.body?.slugs)
    ? req.body.slugs.filter((slug: unknown): slug is string => typeof slug === "string")
    : [];
  if (!language || slugs.length === 0) {
    res.status(400).json({ error: "Invalid translation batch" });
    return;
  }
  let languageCode: string;
  try {
    languageCode = resolveSupportedTranslationLanguage(language);
  } catch {
    res.status(400).json({ error: "Invalid or unsupported language" });
    return;
  }
  try {
    const queued = await new TranslationQueueService().enqueueVisible(slugs, languageCode, email);
    res.status(202).json({ queued });
  } catch {
    res.status(500).json({ error: "Translation queue unavailable" });
  }
}
