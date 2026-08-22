import type { NextApiRequest, NextApiResponse } from "next";
import {
  ArticleTranslationError,
  ArticleTranslationService,
} from "../../../core/ArticleTranslationService";
import {
  generationSafetyIdentifier,
  requestIp,
} from "../../../core/generationIdentity";
import { getServerEmail } from "../../../core/serverSession";
import {
  resolveSupportedTranslationLanguage,
} from "../../../core/translationLanguages";

export default async function handler(
  req: NextApiRequest,
  res: NextApiResponse
) {
  const slug = typeof req.query.slug === "string" ? req.query.slug : "";
  if (!slug) {
    res.status(404).json({ error: "Article not found" });
    return;
  }

  const service = new ArticleTranslationService();
  try {
    if (req.method === "GET") {
      const languages = await service.availableLanguages(slug);
      res.status(200).json({ languages });
      return;
    }
    if (req.method !== "POST") {
      res.status(405).json({ error: "Method not allowed" });
      return;
    }

    const email = await getServerEmail(req, res);
    if (!email) {
      res.status(401).json({ error: "Sign in to generate translations" });
      return;
    }

    const requestedLanguage =
      typeof req.body?.language === "string" ? req.body.language : "";
    let languageCode: string;
    try {
      languageCode = resolveSupportedTranslationLanguage(requestedLanguage);
    } catch {
      res.status(400).json({ error: "Invalid or unsupported language" });
      return;
    }

    const safetyIdentifier = generationSafetyIdentifier(email, requestIp(req));
    const { translation, created } = await service.generate(
      slug,
      languageCode,
      email,
      safetyIdentifier
    );
    res.status(created ? 201 : 200).json({
      languageCode: translation.language_code,
      created,
    });
  } catch (error) {
    if (error instanceof ArticleTranslationError) {
      if (error.retryAfterSeconds) {
        res.setHeader("Retry-After", String(error.retryAfterSeconds));
      }
      res.status(error.statusCode).json({ error: error.message });
      return;
    }
    console.error("Article translation failed", error);
    res.status(500).json({ error: "Translation failed" });
  }
}
