import { image_cache } from "@prisma/client";
import { DateTime } from "luxon";
import { v4 as uuidv4 } from "uuid";
import fs from "node:fs/promises";
import path from "node:path";
import { computeHash } from "./computeHash";
import { GeneratedImageData } from "./ContentGenerator";
import prisma from "./PrismaWibble";

export class ImageRepository {
  public async createEntry(
    contentId: string,
    prompt: string,
    altText: string
  ): Promise<image_cache> {
    return prisma.image_cache.create({
      data: {
        id: uuidv4(),
        content_id: contentId,
        prompt_hash: computeHash(prompt),
        prompt,
        alt_text: altText,
        created_at: DateTime.utc().toJSDate(),
        flagged: false,
        regenerate: false,
        status: "pending",
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
    return prisma.image_cache.findFirst({
      where: {
        flagged: false,
        OR: [{ status: "pending" }, { regenerate: true }],
      },
      orderBy: [{ fail_count: "asc" }, { created_at: "asc" }],
    });
  }

  public async removeEntry(id: string) {
    await prisma.image_cache.delete({ where: { id } });
  }

  public async updateEntryWithImage(
    id: string,
    generated: GeneratedImageData
  ) {
    const imagesRoot = path.resolve(
      process.env.IMAGES_DIR ?? "/home/fabio/services/wibble/images"
    );
    await fs.mkdir(imagesRoot, { recursive: true });
    const relativePath = `${id}${generated.extension}`;
    await fs.writeFile(path.join(imagesRoot, relativePath), generated.image);

    await prisma.$transaction([
      prisma.image_file.upsert({
        where: { id },
        create: { id, file_path: relativePath },
        update: { file_path: relativePath },
      }),
      prisma.image_cache.update({
        where: { id },
        data: {
          status: "completed",
          regenerate: false,
          generator: generated.generator,
          model: generated.model,
          seed: generated.seed,
          parameters: generated.parameters,
          fail_count: 0,
          last_error: null,
          generation_finished_at: DateTime.utc().toJSDate(),
        },
      }),
    ]);
  }

  public async markGenerating(id: string) {
    await prisma.image_cache.update({
      where: { id },
      data: {
        status: "generating",
        generation_started_at: DateTime.utc().toJSDate(),
      },
    });
  }

  public async flagImage(image: image_cache, error?: string) {
    const failCount = image.fail_count + 1;
    await prisma.image_cache.update({
      where: { id: image.id },
      data: {
        fail_count: failCount,
        flagged: failCount >= 5,
        status: failCount >= 5 ? "failed" : "pending",
        last_error: error ?? null,
      },
    });
  }

  public async getImageById(id: string): Promise<image_cache | null> {
    return prisma.image_cache.findUnique({ where: { id } });
  }

  public async getImageByPrompt(
    prompt: string,
    contentId: string,
    altText: string
  ): Promise<image_cache> {
    const existing = await prisma.image_cache.findFirst({
      where: { prompt, flagged: false },
      orderBy: { created_at: "desc" },
    });
    return existing ?? this.createEntry(contentId, prompt, altText);
  }

  public async getLastBlockedImages(): Promise<image_cache[]> {
    return prisma.image_cache.findMany({
      where: { flagged: true },
      orderBy: { created_at: "desc" },
      take: 100,
    });
  }
}
