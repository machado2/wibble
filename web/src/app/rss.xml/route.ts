import { NextResponse } from "next/server";
import { NewsListItem } from "@/core/NewsListItem";
import { computeHash } from "@/core/computeHash";
import { ContentRepository } from "@/core/ContentRepository";

export const dynamic = 'force-dynamic'

function generateRSSFeed(newsItems: NewsListItem[]): string {
  const rssItemsXml = newsItems
    .map((item) => {
      const imageId = item.imagePrompt ? computeHash(item.imagePrompt) : null;
      const mediaUrl = imageId
        ? `https://wibble.news/api/image/${imageId}`
        : null;
      const mediaTag = mediaUrl
        ? `<media:content url="${mediaUrl}" type="image/jpeg" medium="image"><media:description></media:description></media:content>`
        : "";
      return `
        <item>
          <title>${item.title}</title>
          <link>https://wibble.news/content/${item.slug}</link>
          <description>${item.description}</description>
          <pubDate>${item.created_at}</pubDate>
          <guid isPermaLink="false">https://wibble.news/content/${item.slug}</guid>
          ${mediaTag}
        </item>`;
    })
    .join("");

  return `<?xml version="1.0" encoding="UTF-8" ?>
  <rss version="2.0" xmlns:media="http://search.yahoo.com/mrss/">
    <channel>
      <title>Latest News - The Wibble</title>
      <link>https://wibble.news</link>
      <description>Latest news articles from The Wibble</description>
      <language>en</language>
      ${rssItemsXml}
    </channel>
  </rss>`;
}

export async function GET() {
  const service = new ContentRepository();
  const data = await service.getNextPage(
    undefined,
    undefined,
    undefined,
    100,
    undefined,
    undefined,
    undefined
  );
  const xml = generateRSSFeed(data);
  return new NextResponse(xml, {
    headers: {
      "Content-Type": "application/rss+xml",
    },
  });
}
