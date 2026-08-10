import type { NextApiRequest, NextApiResponse } from "next";
import { ContentService } from "@/core/ContentService";
import { getServerEmail } from "@/core/serverSession";

const handler = async (req: NextApiRequest, res: NextApiResponse) => {
  const slug = req.query.path?.[0];
  if (!slug) {
    res.status(404).send("Hum...");
    return;
  }
  try {
    const service = new ContentService();
    const email = await getServerEmail(req, res);
    const data = await service.processSlug(email, slug as string);
    if (!data) {
      res.status(404).send("Not found");
      return;
    }
    res.status(200).send(data);
  } catch (error: any) {
    res.status(500).send("Oups...");
  }
};

export default handler;
