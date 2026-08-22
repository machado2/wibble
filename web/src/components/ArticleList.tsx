import { useState, useEffect } from "react";
import styles from "@/styles/newsList.module.css";
import { NewsListItem } from "@/core/NewsListItem";
import { NewsListItemUI } from "@/components/NewsListItemUI";
import { loadNews, loadNewsOnce } from "@/core/loadNews";
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
  const hasPendingTranslations = Boolean(
    language && news?.some((item) => item.translationState === "pending")
  );

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

  useEffect(() => {
    if (!hasPendingTranslations) return;

    const startedAt = Date.now();
    const pollingWindowMs = 5 * 60 * 1000;
    const maximumDelayMs = 60 * 1000;
    let cancelled = false;
    let delayMs = 5000;
    let timeout: number | undefined;

    const scheduleNext = () => {
      if (cancelled || Date.now() - startedAt >= pollingWindowMs) return;
      timeout = window.setTimeout(refreshTranslations, delayMs);
    };
    const refreshTranslations = async () => {
      if (cancelled || Date.now() - startedAt >= pollingWindowMs) return;
      if (document.hidden) {
        delayMs = Math.min(delayMs * 2, maximumDelayMs);
        scheduleNext();
        return;
      }
      try {
        const refreshedNews = await loadNewsOnce(
          PAGE_SIZE,
          afterId,
          t,
          sort,
          searchTerm,
          model,
          language
        );
        if (!cancelled) setNews(refreshedNews);
      } catch {
        // Keep the current cards visible and retry with backoff.
      } finally {
        delayMs = Math.min(delayMs * 2, maximumDelayMs);
        scheduleNext();
      }
    };

    scheduleNext();
    return () => {
      cancelled = true;
      if (timeout !== undefined) window.clearTimeout(timeout);
    };
  }, [
    hasPendingTranslations,
    searchTerm,
    model,
    afterId,
    t,
    sort,
    language,
  ]);

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
