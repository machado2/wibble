import { image_cache } from "@prisma/client";
import { computeHash } from "./computeHash";
import prisma from "./PrismaWibble";

export class ImageRepository {
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

}
