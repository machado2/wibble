import { image_cache } from "@prisma/client";
import { GeneratedImageData } from "@/worker/ContentGenerator";
import { computeHash } from "./computeHash";
import prisma from "./PrismaWibble";

export class ImageRepository {
  public async createEntry(_prompt: string): Promise<image_cache> {
    throw new Error("Image creation is handled by the native generation queue");
  }

  public async updateEntryWithCreatedAt(id: string) {
    await prisma.image_cache.update({
      where: { id },
      data: { created_at: new Date() },
    });
  }

  public async getNextImageToGenerate(): Promise<image_cache | null> {
    return await prisma.image_cache.findFirst({
      where: {
        flagged: false,
        OR: [{ status: "pending" }, { regenerate: true }],
      },
      orderBy: { created_at: "asc" },
    });
  }

  public async removeEntry(id: string) {
    await prisma.image_cache.delete({ where: { id } });
  }

  public async updateEntryWithImage(
    _id: string,
    _genImgData: GeneratedImageData
  ) {
    throw new Error("Image persistence is handled by the native image store");
  }

  public async flagImage(id: string) {
    await prisma.image_cache.update({
      where: { id },
      data: { flagged: true },
    });
  }

  public async getImageById(id: string): Promise<image_cache | null> {
    const direct = await prisma.image_cache.findUnique({ where: { id } });
    if (direct) return direct;

    const hashed = await prisma.image_cache.findFirst({
      where: { prompt_hash: id },
    });
    if (hashed) return hashed;

    const candidates = await prisma.image_cache.findMany({
      where: { flagged: false },
      orderBy: { created_at: "desc" },
      take: 500,
    });
    return candidates.find((image) => computeHash(image.prompt) === id) ?? null;
  }

  public async getImageByPrompt(prompt: string): Promise<image_cache | null> {
    return await prisma.image_cache.findFirst({
      where: { prompt, flagged: false },
      orderBy: { created_at: "desc" },
    });
  }

  public async getLastBlockedImages(): Promise<image_cache[]> {
    return await prisma.image_cache.findMany({
      where: { flagged: true },
      orderBy: { created_at: "desc" },
      take: 100,
    });
  }
}
