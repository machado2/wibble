import { requireSession } from "@/core/serverSession";
import prisma from "@/core/PrismaWibble";
import fs from "fs/promises";
import path from "path";
import { IncomingForm } from "formidable-serverless";
import type { NextApiRequest, NextApiResponse } from "next";

export const config = {
  api: { bodyParser: false, sizeLimit: "10mb" },
};

const handler = async (req: NextApiRequest, res: NextApiResponse) => {
  if (!(await requireSession(req, res))) return;
  if (req.method !== "PUT") {
    res.setHeader("Allow", "PUT");
    res.status(405).end(`Method ${req.method} Not Allowed`);
    return;
  }

  const id = Array.isArray(req.query.id) ? req.query.id[0] : req.query.id;
  if (!id) {
    res.status(400).json({ error: "Missing image id" });
    return;
  }

  try {
    const files = await new Promise<Record<string, any>>((resolve, reject) => {
      new IncomingForm().parse(req, (error, _fields, parsedFiles) => {
        if (error) reject(error);
        else resolve(parsedFiles);
      });
    });
    const file = Array.isArray(files.file) ? files.file[0] : files.file;
    if (!file?.path) {
      res.status(400).json({ error: "No file provided" });
      return;
    }

    const extension = path.extname(file.name ?? "").toLowerCase();
    const safeExtension = [".jpg", ".jpeg", ".png", ".webp"].includes(extension)
      ? extension
      : ".jpg";
    const root = path.resolve(
      process.env.IMAGES_DIR ?? "/home/fabio/services/wibble/images"
    );
    const relativePath = `${id}${safeExtension}`;
    await fs.mkdir(root, { recursive: true });
    await fs.copyFile(file.path, path.join(root, relativePath));

    await prisma.$transaction([
      prisma.image_file.upsert({
        where: { id },
        create: { id, file_path: relativePath },
        update: { file_path: relativePath },
      }),
      prisma.image_cache.update({
        where: { id },
        data: { status: "completed", regenerate: false },
      }),
    ]);
    res.status(200).json({ id });
  } catch (error) {
    console.error(error);
    res.status(500).json({ error: "Failed to update the image" });
  }
};

export default handler;
