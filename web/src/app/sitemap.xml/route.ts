import prisma from "@/core/PrismaWibble";
import xml from "xml";
import { NextResponse } from "next/server";

export const dynamic = 'force-dynamic'

const HIGH_PRIORITY = 1;
const DEFAULT_PRIORITY = 0.1;

export async function GET() {
  // articles where the content is not null and are not flagged
  const articles = await prisma.content.findMany({
    select: {
      slug: true,
      model: true,
      created_at: true,
      votes: true,
    },
    where: {
      content: {
        not: null,
      },
      flagged: false,
    },
    orderBy: {
      created_at: "desc",
    },
  });

  const namespace = {
    _attr: {
      xmlns: "http://www.sitemaps.org/schemas/sitemap/0.9",
    },
  };

  let urlset: any[] = [namespace];
  for (let article of articles) {
    const fullUrl = `https://wibble.news/content/${article.slug}`;
    // we give higher priority to GPT-4 articles and articles that were voted on
    const priority =
      article.model.includes("gpt-4") || article.votes > 0
        ? HIGH_PRIORITY
        : DEFAULT_PRIORITY;
    urlset.push({
      url: [
        {
          loc: fullUrl,
        },
        {
          lastmod: article.created_at.toISOString(),
        },
        { priority: priority },
      ],
    });
  }

  const xmlSitemap = xml([{ urlset }], { declaration: true });
  return new NextResponse(xmlSitemap, {
    headers: {
      "Content-Type": "application/xml",
    },
  });
}
