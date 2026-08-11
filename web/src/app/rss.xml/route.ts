import { NextResponse } from "next/server";
import { NewsListItem } from "@/core/NewsListItem";
import { computeHash } from "@/core/computeHash";
import { ContentRepository } from "@/core/ContentRepository";
import { getWibbleConfig } from "../../../../config-runtime";

export const dynamic = "force-dynamic";

const escapeXml = (value: string) =>
  value
    .replace(/&/g, "&amp;")
    .replace(/</g, "&lt;")
    .replace(/>/g, "&gt;")
    .replace(/"/g, "&quot;")
    .replace(/'/g, "&apos;");

function generateRSSFeed(newsItems: NewsListItem[], siteUrl: string): string {
  const rssItemsXml = newsItems
    .map((item) => {
      const imageId = item.imagePrompt ? computeHash(item.imagePrompt) : null;
      const mediaUrl = imageId ? `${siteUrl}/api/image/${imageId}` : null;
      const mediaTag = mediaUrl
        ? `<media:content url="${mediaUrl}" medium="image"><media:description>${escapeXml(
            item.imagePrompt
          )}</media:description></media:content>`
        : "";
      return `
        <item>
          <title>${escapeXml(item.title)}</title>
          <link>${siteUrl}/content/${encodeURIComponent(item.slug)}</link>
          <description>${escapeXml(item.description)}</description>
          <pubDate>${new Date(item.created_at).toUTCString()}</pubDate>
          <guid isPermaLink="true">${siteUrl}/content/${encodeURIComponent(
        item.slug
      )}</guid>
          ${mediaTag}
        </item>`;
    })
    .join("");

  return `<?xml version="1.0" encoding="UTF-8" ?>
  <rss version="2.0" xmlns:media="http://search.yahoo.com/mrss/">
    <channel>
      <title>Latest News - The Wibble</title>
      <link>${siteUrl}</link>
      <description>Latest news articles from The Wibble</description>
      <language>en</language>
      ${rssItemsXml}
    </channel>
  </rss>`;
}

export async function GET() {
  const siteUrl = getWibbleConfig().app.site_url.replace(/\/$/, "");
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
  const xml = generateRSSFeed(data, siteUrl);
  return new NextResponse(xml, {
    headers: {
      "Content-Type": "application/rss+xml",
    },
  });
}
