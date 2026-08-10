import { NextApiRequest, NextApiResponse } from "next";
import { ContentService } from "@/core/ContentService";
import { getServerEmail } from "@/core/serverSession";

const replaceEmptyUndefined = (t: string | null | undefined): string | undefined => {
  if (!t) return undefined;
  if (t.trim() === "") return undefined;
  return t.trim();
};

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
    const model =
      email && email === adminEmail
        ? replaceEmptyUndefined(req.body?.model)
        : undefined;

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
      model
    );
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
