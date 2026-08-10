import prisma from "./PrismaWibble";
import logger from "./logger";
import { SleepUpdateScores } from "./sleep";

/*
const BATCH_SIZE = 100; // Choose an appropriate batch size based on your data and performance
const G = 1.8;

function calculateHotScore(votes: number, createdAt: Date) {
    const ageInHours = (new Date().getTime() - createdAt.getTime()) / 3600 / 1000;
    const numerator = Math.max(votes + 5, 0);
    return numerator / Math.pow((ageInHours + 2), G);
}

async function processBatch(prisma: PrismaClient, lastCreatedAt: Date | null, lastId: string | null)
    : Promise<[Date | null, string | null]> {
    const contents = await prisma.content.findMany({
        where: lastCreatedAt
            ? {
                OR: [
                    { created_at: { gt: lastCreatedAt } },
                    {
                        AND: [
                            { created_at: { gte: lastCreatedAt } },
                            { id: { gt: lastId || '' } },
                        ],
                    },
                ],
            }
            : undefined,
        orderBy: [
            { created_at: 'asc' },
            { id: 'asc' },
        ],
        take: BATCH_SIZE,
        select: {
            id: true,
            votes: true,
            created_at: true,
        },
    });

    if (contents.length === 0) {
        return [null, null];
    }

    const updatePromises = contents.map(content => {
        const score = calculateHotScore(content.votes, content.created_at);
        return prisma.content.update({
            where: { id: content.id },
            data: { hot_score: score },
        });
    });

    await Promise.all(updatePromises);

    return [contents[contents.length - 1].created_at, contents[contents.length - 1].id];
}

export async function updateHotScores() {
    let lastCreatedAt: Date | null = null;
    let lastId: string | null = null;

    try {
        console.log('Updating hot scores');
        do {
            console.log(`Processing batch starting at ${lastCreatedAt} ${lastId}`);
            try {
                [lastCreatedAt, lastId] = await processBatch(prisma, lastCreatedAt, lastId);
            } catch (error) {
                console.log(`Error processing batch: ${error}`);
                // sleep for 30 seconds
                await new Promise((resolve) => setTimeout(resolve, 30000));
            }
        } while (lastCreatedAt && lastId);
    } catch (error) {
        console.log(error);
    }
}

*/

async function updateHotScores() {
  await prisma.$executeRaw`
    UPDATE content
    SET hot_score = GREATEST(votes + 3, 0)
        / POWER(
            GREATEST(
              EXTRACT(EPOCH FROM ((CURRENT_TIMESTAMP AT TIME ZONE 'UTC') - created_at))
                / 3600.0 + 2,
              1
            ),
            0.1
          )`;
}

export async function updateScoresLoop() {
  while (true) {
    try {
      await updateHotScores();
    } catch (error) {
      logger.error(`Error updating hot scores: ${error}`);
    }
    await SleepUpdateScores();
  }
}
