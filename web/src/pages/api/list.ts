import { NextApiRequest, NextApiResponse } from "next";
import { getServerEmail } from "@/core/serverSession";
import { ContentRepository } from "@/core/ContentRepository";
import { resolveSupportedTranslationLanguage } from "@/core/translationLanguages";

const parseInteger = (
  value: string | string[] | undefined
): number | undefined => {
  if (!value) return 0;
  const parsed = parseInt(value as string);
  if (isNaN(parsed)) return 0;
  return parsed;
};

const parseString = (s: string | string[] | undefined): string | undefined => {
  if (!s) return undefined;
  if (Array.isArray(s)) return s[0];
  return s as string;
};

export default async function handler(
  req: NextApiRequest,
  res: NextApiResponse
) {
  try {
    const email = await getServerEmail(req, res);
    const { query } = req;
    const repo = new ContentRepository();
    const period = (query.t as string) ?? undefined;
    const days = [undefined, 7, 30][["week", "month"].indexOf(period) + 1];
    let languageCode: string | undefined;
    const requestedLanguage = parseString(query.lang);
    if (requestedLanguage) {
      try {
        languageCode = resolveSupportedTranslationLanguage(requestedLanguage);
      } catch {
        languageCode = undefined;
      }
    }
    const data = await repo.getNextPage(
      parseString(query.afterId),
      parseString(query.sort),
      days,
      parseInteger(query.pageSize),
      parseString(query.search),
      email ?? undefined,
      parseString(query.model),
      languageCode
    );

    res.status(200).json(data);
  } catch (error) {
    console.error(error);
    res.status(500).json({ message: "Error reading latest news" });
  }
}
