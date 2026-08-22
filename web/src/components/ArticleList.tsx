import { useState, useEffect } from "react";
import styles from "@/styles/newsList.module.css";
import { NewsListItem } from "@/core/NewsListItem";
import { NewsListItemUI } from "@/components/NewsListItemUI";
import { loadNews } from "@/core/loadNews";
import Link from "next/link";
import { useRouter } from "next/router";
import { dontWaitFor } from "@/core/dontWaitFor";
import { useGlobalLanguage } from "./GlobalLanguage";

const PAGE_SIZE = 20;

export type ArticleListProps = {
  latestNews: NewsListItem[] | undefined;
};

export default function ArticleList(props: ArticleListProps) {
  const router = useRouter();
  const { copy } = useGlobalLanguage();
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
  const language = readString("lang");

  useEffect(() => {
    const reload = async () => {
      setNews(undefined);
      const newNews = await loadNews(
        PAGE_SIZE,
        afterId,
        t,
        sort,
        searchTerm,
        model,
        language
      );
      setNews(newNews);
    };

    if (props.latestNews) {
      setNews(props.latestNews);
    } else {
      dontWaitFor(reload());
    }
  }, [props.latestNews, searchTerm, model, afterId, t, sort, language]);

  if (!news) {
    return <p className={styles.loader}>{copy.loadingArticles}</p>;
  }

  return (
    <>
      {news.length === 0 ? (
        <p className={styles.noMoreNews}>{copy.noArticles}</p>
      ) : null}
      {news.map((newsItem) => (
        <div key={`${newsItem.id}`} className={styles.newsItem}>
          <NewsListItemUI item={newsItem} key={newsItem.id} />
        </div>
      ))}
      {news.length === PAGE_SIZE ? (
        <Link
          className={styles.nextPageLink}
          href={{
            pathname: router.pathname,
            query: { ...router.query, afterId: news[news.length - 1].id },
          }}
        >
          {copy.nextPage}
        </Link>
      ) : null}
    </>
  );
}
