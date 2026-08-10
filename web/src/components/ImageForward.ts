import { ImageRepository } from "@/core/ImageRepository";
import prisma from "@/core/PrismaWibble";
import { image_cache } from "@prisma/client";
import fs from "fs/promises";
import path from "path";
import type { NextApiRequest, NextApiResponse } from "next";

type ImagePayload = { data: Buffer; contentType: string };

const localImage = async (image: image_cache): Promise<ImagePayload | null> => {
  const stored = await prisma.image_file.findUnique({ where: { id: image.id } });
  if (!stored) return null;

  const root = path.resolve(process.env.IMAGES_DIR ?? "/home/fabio/services/wibble/images");
  const filePath = path.resolve(root, stored.file_path);
  if (filePath !== root && !filePath.startsWith(`${root}${path.sep}`)) {
    throw new Error("Invalid image path");
  }

  const extension = path.extname(filePath).toLowerCase();
  const contentType =
    extension === ".png"
      ? "image/png"
      : extension === ".webp"
      ? "image/webp"
      : "image/jpeg";
  return { data: await fs.readFile(filePath), contentType };
};

const replicateImage = async (
  image: image_cache
): Promise<ImagePayload | null> => {
  const token = process.env.REPLICATE_API_TOKEN;
  if (!token || !image.provider_job_url) return null;

  const predictionResponse = await fetch(image.provider_job_url, {
    headers: { Authorization: `Token ${token}` },
  });
  if (!predictionResponse.ok) return null;

  const prediction = await predictionResponse.json();
  const output = Array.isArray(prediction?.output)
    ? prediction.output[0]
    : prediction?.output;
  if (typeof output !== "string" || !output.startsWith("https://")) {
    return null;
  }

  const imageResponse = await fetch(output);
  if (!imageResponse.ok) return null;
  return {
    data: Buffer.from(await imageResponse.arrayBuffer()),
    contentType: imageResponse.headers.get("content-type") ?? "image/jpeg",
  };
};

const placeholderImage = async (): Promise<ImagePayload> => ({
  data: await fs.readFile(path.join(process.cwd(), "public", "placeholder.jpeg")),
  contentType: "image/jpeg",
});

const serveImage = async (
  image: image_cache | null | undefined,
  res: NextApiResponse
) => {
  if (!image || image.flagged) {
    res.setHeader("Cache-Control", "no-store");
    res.status(404).send("Image not found");
    return;
  }
  if (image.status !== "completed") {
    res.setHeader("Cache-Control", "no-store");
    res.setHeader("Retry-After", "5");
    res.status(503).send("Image not ready");
    return;
  }

  const payload =
    (await localImage(image)) ??
    (await replicateImage(image)) ??
    (await placeholderImage());

  void prisma.image_cache
    .update({
      where: { id: image.id },
      data: { view_count: { increment: 1 } },
    })
    .catch(() => undefined);

  res.setHeader("Content-Type", payload.contentType);
  res.setHeader("Cache-Control", "public, max-age=3600");
  res.status(200).send(payload.data);
};

const unarray = (value: string | string[] | null | undefined) =>
  Array.isArray(value) ? value[0] ?? null : value;

export const imageHandler = async (
  req: NextApiRequest,
  res: NextApiResponse
) => {
  const id = unarray(req.query.id);
  const hash = unarray(req.query.hash);
  const imageRepo = new ImageRepository();

  if (req.method !== "GET") {
    res.setHeader("Allow", "GET");
    res.status(405).send("Method not allowed");
  } else if (typeof id === "string") {
    await serveImage(await imageRepo.getImageById(id), res);
  } else if (typeof hash === "string") {
    await serveImage(await imageRepo.getImageById(hash), res);
  } else {
    res.status(404).send("Not found");
  }
};
