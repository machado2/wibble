// pages/api/image/[id].ts
import { NextApiRequest, NextApiResponse } from "next";
import { IncomingForm } from "formidable-serverless";
import fs from "fs/promises";
import prisma from "@/core/PrismaWibble";
import { requireSession } from "@/core/serverSession";

export const config = {
  api: {
    bodyParser: false,
    sizeLimit: "10mb",
  },
};

async function handlePut(req: NextApiRequest, res: NextApiResponse) {
  const {
    query: { id },
  } = req;

  try {
    const form = new IncomingForm();

    const fileData = await new Promise<{ [key: string]: any }>(
      (resolve, reject) => {
        form.parse(req, (err, fields, files) => {
          if (err) {
            reject(err);
            return;
          }
          resolve(files);
        });
      }
    );

    const file = fileData.file;
    if (!file) {
      res.status(400).json({ error: "No file provided" });
      return;
    }

    // Read the file into a buffer
    const fileBuffer = await fs.readFile(file.path);

    const updatedImage = await prisma.image_cache.update({
      where: { id: id as string },
      data: { image_data: fileBuffer },
    });

    res.status(200).json(updatedImage);
  } catch (error) {
    console.error(error);
    res.status(500).json({ error: "Failed to update the image" });
  }
}

const handler = async (req: NextApiRequest, res: NextApiResponse) => {
  await requireSession(req, res);
  if (req.method === "PUT") {
    handlePut(req, res);
  } else {
    res.setHeader("Allow", "PUT");
    res.status(405).end(`Method ${req.method} Not Allowed`);
  }
};

export default handler;