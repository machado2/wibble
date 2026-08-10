import prisma from "./PrismaWibble";

let cachedTotal: number | null = null;
let cacheExpiry = Date.now();

const cacheDuration = 24 * 60 * 60 * 1000; // one day

export const getExampleArticles = async () => {
  try {
    // Refresh the cached total if it's null or the cache has expired
    if (cachedTotal === null || Date.now() > cacheExpiry) {
      cachedTotal = await prisma.content.count({
        where: {
          OR: [{ model: "gpt-4" }, { model: "chatgpt-4" }],
          flagged: false,
          generating: false,
          content: { not: null },
        },
      });
      cacheExpiry = Date.now() + cacheDuration;
    }

    if (cachedTotal === 0) {
      return [];
    }

    // Fetch 3 random articles
    const articles = await Promise.all(
      Array.from({ length: 3 }).map(async () => {
        const randomOffset = Math.floor(Math.random() * cachedTotal!);
        return prisma.content.findFirst({
          where: {
            OR: [{ model: "gpt-4" }, { model: "chatgpt-4" }],
            flagged: false,
            generating: false,
            content: { not: null },
          },
          skip: randomOffset,
          take: 1,
          select: {
            content: true,
          },
        });
      })
    );

    return articles.flatMap((article) =>
      article?.content ? [article.content] : []
    );
  } catch (error) {
    console.error("Error fetching example articles:", error);
    return [];
  }
};
