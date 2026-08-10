import { image_cache } from "@prisma/client";
import { DateTime } from "luxon";
import { computeHash } from "./computeHash";
import { GeneratedImageData } from "./ContentGenerator";

import prisma from "./PrismaWibble";

export class ImageRepository {
  public async createEntry(prompt: string): Promise<image_cache> {
    const id = computeHash(prompt);
    return await prisma.image_cache.create({
      data: {
        id,
        prompt,
        flagged: false,
        created_at: DateTime.utc().toJSDate(),
        fail_count: 0,
      },
    });
  }

  public async updateEntryWithCreatedAt(id: string) {
    await prisma.image_cache.update({
      where: { id },
      data: { created_at: DateTime.utc().toJSDate() },
    });
  }

  public async getNextImageToGenerate(): Promise<image_cache | null> {
    return await prisma.image_cache.findFirst({
      where: {
        flagged: false,
        OR: [{ image_data: null }, { regenerate: true }],
      },
      orderBy: [{ fail_count: "asc" }, { created_at: "asc" }],
    });
  }

  public async removeEntry(id: string) {
    await prisma.image_cache.delete({
      where: { id },
    });
  }

  public async updateEntryWithImage(
    id: string,
    genImgData: GeneratedImageData
  ) {
    await prisma.image_cache.update({
      where: { id },
      data: {
        image_data: genImgData.image,
        model: genImgData.model,
        generator: genImgData.generator,
        regenerate: false,
        seed: genImgData.seed,
        parameters: genImgData.parameters,
      },
    });
  }

  public async flagImage(image: image_cache) {
    const fail_count = image.fail_count + 1;
    const flagged = fail_count >= 5;
    await prisma.image_cache.update({
      where: { id: image.id },
      data: {
        fail_count,
        flagged,
      },
    });
  }

  public async getImageById(id: string): Promise<image_cache | null> {
    return await prisma.image_cache.findUnique({
      where: { id },
    });
  }

  public async getImageByPrompt(prompt: string): Promise<image_cache> {
    const id = computeHash(prompt);
    const existingImage = await this.getImageById(id);
    if (existingImage) {
      return existingImage;
    }
    return await this.createEntry(prompt);
  }

  public async getLastBlockedImages(): Promise<image_cache[]> {
    return await prisma.image_cache.findMany({
      where: { flagged: true },
      orderBy: { created_at: "desc" },
      take: 100,
    });
  }
}
