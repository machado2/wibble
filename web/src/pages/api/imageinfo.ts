import { ImageRepository } from "@/core/ImageRepository";
import { image_cache } from "@prisma/client";
import type { NextApiRequest, NextApiResponse } from "next";
import prisma from "@/core/PrismaWibble";

export type ImageInfoResponse = {
  id: string;
  prompt: string;
  seed: string | null;
  model: string | null;
  created_at: Date;
};

const serveImage = (
  image: image_cache | null | undefined,
  res: NextApiResponse
) => {
  if (!image || image.flagged || !image.image_data) {
    res.status(503).send("Image not ready");
    return;
  }

  prisma.image_cache
    .update({
      where: { id: image.id },
      data: { view_count: { increment: 1 } },
    })
    .catch((err: any) => {
      console.error(err);
    });

  let prompt: string;
  if (image?.parameters) {
    const params = JSON.parse(image.parameters);
    prompt = params?.prompt ?? image.prompt;
  } else {
    prompt = image.prompt;
  }
  const response: ImageInfoResponse = {
    id: image.id,
    prompt,
    seed: image.seed,
    model: image.model,
    created_at: image.created_at,
  };
  res.status(200).send(response);
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

const handler = async (req: NextApiRequest, res: NextApiResponse) => {
  const id = unarray(req.query.id);
  const body = req.body?.length > 0 ? JSON.parse(req.body) : null;
  const prompt = body?.prompt ?? null;
  if (typeof prompt === "string") {
    await serveImageByPrompt(prompt, req, res);
  } else if (typeof id === "string") {
    await serveImageById(id, req, res);
  } else {
    res.status(404).send("Not found");
  }
};

export default handler;
