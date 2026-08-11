import { NextApiRequest, NextApiResponse } from "next";
import { ContentService } from "@/core/ContentService";
import { getServerEmail, isServerAdmin } from "@/core/serverSession";
import { normalizeArticleTarget } from "@/core/articleTarget";
import { NotFoundRepository } from "@/core/NotFoundRepository";
import {
  generationSafetyIdentifier,
  requestIp,
} from "@/core/generationIdentity";
import { selectWriter, WriterSelectionError } from "@/core/writers";
import { getWibbleConfig } from "../../../../config-runtime";

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
    const config = getWibbleConfig();
    const admin = isServerAdmin(email);

    const prompt =
      typeof req.body?.prompt === "string" ? req.body.prompt.trim() : "";
    const requestedWriter =
      typeof req.body?.writer === "string" ? req.body.writer.trim() : undefined;
    const targetUrl =
      typeof req.body?.url === "string" ? req.body.url.trim() : "";
    if (targetUrl && !admin) {
      res.status(403).json({ error: "Only administrators can select a URL" });
      return;
    }
    let target: ReturnType<typeof normalizeArticleTarget> | undefined;
    if (targetUrl) {
      try {
        target = normalizeArticleTarget(targetUrl, config.app.site_url);
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
    if (prompt.length > config.generation.max_prompt_length) {
      res.status(400).json({ error: "Your text is too long!" });
      return;
    }

    const writer = selectWriter(
      config.generation.writers,
      requestedWriter || undefined,
      admin
    );

    const service = new ContentService();
    const safetyIdentifier = generationSafetyIdentifier(email, requestIp(req));
    const content = await service.generateForSuggestion(
      email,
      prompt,
      writer,
      target?.slug,
      safetyIdentifier
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
    if (error instanceof WriterSelectionError) {
      res.status(error.statusCode).json({ error: error.message });
      return;
    }
    res.status(500).json({ error: error?.message ?? "Internal server error" });
  }
}
