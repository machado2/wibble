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
    const email = await getServerEmail(req, res);

    if (req.method !== "POST") {
      res.status(405).json({ message: "Method not allowed" });
      return;
    }

    const prompt = req.body?.prompt;
    const model = email ? replaceEmptyUndefined(req.body?.model) : undefined;

    if (!prompt) {
      res.status(400).json({ message: "Missing prompt" });
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
      .json({ message: error?.message ?? "Internal server error" });
  }
}
