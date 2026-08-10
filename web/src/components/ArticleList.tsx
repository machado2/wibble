import { useState, useEffect } from "react";
import styles from "@/styles/newsList.module.css";
import { NewsListItem } from "@/core/NewsListItem";
import { NewsListItemUI } from "@/components/NewsListItemUI";
import { loadNews } from "@/core/loadNews";
import Link from "next/link";
import { useRouter } from "next/router";
import { dontWaitFor } from "@/core/dontWaitFor";

const PAGE_SIZE = 20;

export type ArticleListProps = {
  latestNews: NewsListItem[] | undefined;
};

export default function ArticleList(props: ArticleListProps) {
  const router = useRouter();
  const [news, setNews] = useState<NewsListItem[] | undefined>(
    props.latestNews
  );

  const readString = (p: string): string | undefined => {
    const str = router.query[p];
    if (str === undefined) return undefined;
    if (typeof str === "string") return str;
    return str[0];
  };

  const searchTerm = readString("search");
  const model = readString("model");
  const afterId = readString("afterId");
  const t = readString("t");
  const sort = readString("sort");

  useEffect(() => {
    const reload = async () => {
      const newNews = await loadNews(
        PAGE_SIZE,
        afterId,
        t,
        sort,
        searchTerm,
        model
      );
      setNews(newNews);
    };

    if (!props.latestNews) {
      dontWaitFor(reload());
    }
  }, [searchTerm, model, afterId, t, sort]);

  if (!news) {
    return null;
  }

  return (
    <>
      {news.map((newsItem) => (
        <div key={`${newsItem.id}`} className={styles.newsItem}>
          <NewsListItemUI item={newsItem} key={newsItem.id} />
        </div>
      ))}
      {news.length > 0 ? (
        <Link
          className={styles.nextPageLink}
          href={{
            pathname: router.pathname,
            query: { ...router.query, afterId: news[news.length - 1].id },
          }}
        >
          Next Page
        </Link>
      ) : null}
    </>
  );
}
