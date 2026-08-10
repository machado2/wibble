import { DateTime } from "luxon";
import { v4 as uuidv4 } from "uuid";
import prisma from "./PrismaWibble";

export class NotFoundRepository {
  async record(url: string): Promise<void> {
    const now = DateTime.utc().toJSDate();
    await prisma.not_found_request.upsert({
      where: { url },
      create: {
        id: uuidv4(),
        url,
        hit_count: 1,
        first_seen_at: now,
        last_seen_at: now,
      },
      update: {
        hit_count: { increment: 1 },
        last_seen_at: now,
      },
    });
  }

  async markRequested(url: string, slug: string): Promise<void> {
    await prisma.not_found_request.updateMany({
      where: { url },
      data: {
        generated_slug: slug,
      },
    });
  }
}
