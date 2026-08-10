import { ImageRepository } from "@/core/ImageRepository";
import { image_cache } from "@prisma/client";
import type { NextApiRequest, NextApiResponse } from "next";

const serveImage = (
  image: image_cache | null | undefined,
  res: NextApiResponse
) => {
  if (!image || image.flagged || !image.image_data) {
    res.status(503).send("Image not ready");
    return;
  }
  res.setHeader("Content-Type", "image/jpeg");
  res.status(200).send(image.image_data);
  res.end();
};

const serveImageByPrompt = async (
  prompt: string,
  req: NextApiRequest,
  res: NextApiResponse
) => {
  const imageRepo = new ImageRepository();
  const image = await imageRepo.getImageByPrompt(prompt);
  serveImage(image, res);
};

const serveImageById = async (
  id: string,
  req: NextApiRequest,
  res: NextApiResponse
) => {
  const imageRepo = new ImageRepository();
  const image = await imageRepo.getImageById(id);
  if (!image) {
    res.status(404).send("Not found");
    return;
  }
  serveImage(image, res);
};

const unarray = (value: string | string[] | null | undefined) => {
  if (Array.isArray(value)) {
    if (value.length === 0) {
      return null;
    }
    return value[0];
  }
  return value;
};

export const imageHandler = async (
  req: NextApiRequest,
  res: NextApiResponse
) => {
  const id = unarray(req.query.id);
  const hash = unarray(req.query.hash);
  const body = req.body?.length > 0 ? JSON.parse(req.body) : null;
  const prompt = body?.prompt ?? null;
  if (typeof prompt === "string") {
    await serveImageByPrompt(prompt, req, res);
  } else if (typeof id === "string") {
    await serveImageById(id, req, res);
  } else if (typeof hash === "string") {
    await serveImageById(hash, req, res);
  } else {
    res.status(404).send("Not found");
  }
};
