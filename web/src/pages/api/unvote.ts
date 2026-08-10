import { ContentService } from "@/core/ContentService";
import { getServerEmail } from "@/core/serverSession";
import { NextApiRequest, NextApiResponse } from "next";

export default async function handler(
  req: NextApiRequest,
  res: NextApiResponse
) {
  const email = await getServerEmail(req, res);
  if (req.method !== "POST") {
    res.status(405).json({ message: "Method not allowed" });
    return;
  }
  if (!email) {
    res.status(401).json({ message: "Unauthorized" });
    return;
  }
  const contentId = req.body?.content_id as string;
  const service = new ContentService();
  await service.unvote(contentId, email);
  res.status(200).send("OK");
}
