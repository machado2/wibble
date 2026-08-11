import { NextApiRequest, NextApiResponse } from "next";
import { ContentService } from "@/core/ContentService";
import { getServerEmail } from "@/core/serverSession";
import { normalizeArticleTarget } from "@/core/articleTarget";
import { NotFoundRepository } from "@/core/NotFoundRepository";

export default async function handler(
  req: NextApiRequest,
  res: NextApiResponse
) {
  try {
    if (req.method !== "POST") {
      res.status(405).json({ error: "Method not allowed" });
      return;
    }
    const email = await getServerEmail(req, res);

    const prompt =
      typeof req.body?.prompt === "string" ? req.body.prompt.trim() : "";
    const adminEmail =
      process.env.ADMIN_EMAIL ?? process.env.REACT_ADMIN_EMAIL ?? "";
    const targetUrl =
      typeof req.body?.url === "string" ? req.body.url.trim() : "";
    if (targetUrl && (!email || email !== adminEmail)) {
      res.status(403).json({ error: "Only administrators can select a URL" });
      return;
    }
    let target: ReturnType<typeof normalizeArticleTarget> | undefined;
    if (targetUrl) {
      try {
        target = normalizeArticleTarget(
          targetUrl,
          process.env.NEXT_PUBLIC_SITE_URL ??
            process.env.SITE_URL ??
            "https://wibble.fbmac.net",
        );
      } catch (targetError) {
        res.status(400).json({
          error:
            targetError instanceof Error
              ? targetError.message
              : "Invalid target URL",
        });
        return;
      }
    }
    if (!prompt) {
      res.status(400).json({ error: "Missing prompt" });
      return;
    }
    if (prompt.length > 2000) {
      res.status(400).json({ error: "Your text is too long!" });
      return;
    }

    const service = new ContentService();
    const content = await service.generateForSuggestion(
      email,
      prompt,
      target?.slug,
    );
    if (target) {
      await new NotFoundRepository().markRequested(target.url, content.slug);
    }
    const data = {
      slug: content.slug,
      title: content.title,
      description: content.description,
    };
    res.status(200).json(data);
  } catch (error: any) {
    console.error(error);
    res
      .status(500)
      .json({ error: error?.message ?? "Internal server error" });
  }
}
