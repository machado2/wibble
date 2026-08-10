import { ContentService } from "@/core/ContentService";
import { getServerEmail } from "@/core/serverSession";
import { NextApiRequest, NextApiResponse } from "next";

export default async function handler(
  req: NextApiRequest,
  res: NextApiResponse
) {
  if (req.method !== "POST") {
    res.status(405).json({ message: "Method not allowed" });
    return;
  }
  try {
    const email = await getServerEmail(req, res);
    if (!email) {
      res.status(401).json({ message: "Unauthorized" });
      return;
    }
    const contentId = req.body?.content_id;
    if (typeof contentId !== "string" || !contentId.trim()) {
      res.status(400).json({ message: "Missing content ID" });
      return;
    }
    const service = new ContentService();
    await service.unvote(contentId, email);
    res.status(200).send("OK");
  } catch (error) {
    console.error("Failed to remove vote", error);
    res.status(500).json({ message: "Vote could not be removed" });
  }
}
