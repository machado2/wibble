import { Card } from "antd";
import { useState } from "react";
import styles from "@/styles/newsList.module.css";
import Head from "next/head";
import {
  GetServerSidePropsContext,
  NextApiRequest,
  NextApiResponse,
} from "next";
import { NewsListItem } from "@/core/NewsListItem";
import { getServerEmail } from "@/core/serverSession";
import ArticleList from "@/components/ArticleList";
import { ContentRepository } from "@/core/ContentRepository";
import { useRouter } from "next/router";
import SearchBox from "@/components/SearchBox";
import { SortSelection } from "@/components/SortSelection";
import { NewsImageSelector } from "@/components/NewsImageSelector";
import { TimeSelection } from "@/components/TimeSelection";

const siteDescription = `Get the latest news with a touch of wobble from The Wibble,
your source for the unpredictable and unsteady world of current events.`;
const siteTitle = "The Wibble";
const PAGE_SIZE = 20;

type HomeProps = {
  latestNews: NewsListItem[];
};

export default function Home(props: HomeProps) {
  const [news, setNews] = useState<NewsListItem[] | undefined>(
    props.latestNews
  );
  const router = useRouter();
  const firstRoute = useState<string>(router.asPath)[0];

  if (news && router.asPath !== firstRoute) {
    setNews(undefined);
  }

  const sortOptions = [
    {
      value: "recent",
      label: "New",
    },
    {
      value: "most_voted",
      label: "Votes",
    },
    {
      value: "most_viewed",
      label: "Views",
    },
  ];

  return (
    <>
      <Head>
        <title>{siteTitle || "The Wibble"}</title>
        <meta property="og:title" content={siteTitle} />
        <meta property="og:description" content={siteDescription} />
      </Head>
      <Card className={styles.maincard} bordered={false}>
        <div className={styles.selectors}>
          <NewsImageSelector />
        </div>
        <div className={styles.selectors}>
          <SortSelection options={sortOptions} />
          <TimeSelection />
        </div>
        <SearchBox />
        <ArticleList latestNews={news} />
      </Card>
    </>
  );
}

export async function getServerSideProps(context: GetServerSidePropsContext) {
  try {
    const repo = new ContentRepository();
    const searchTerm = (context.query.search as string) ?? null;
    const model = (context.query.model as string) ?? undefined;
    const email = await getServerEmail(
      context.req as NextApiRequest,
      context.res as NextApiResponse
    );

    const afterId = (context.query.afterId as string) ?? undefined;
    const sort = (context.query.sort as string) ?? undefined;
    const period = (context.query.t as string) ?? undefined;
    const days = [undefined, 7, 30][["week", "month"].indexOf(period) + 1];
    const latestNews: NewsListItem[] = await repo.getNextPage(
      afterId,
      sort,
      days,
      PAGE_SIZE,
      searchTerm,
      email ?? undefined,
      model
    );

    const props: HomeProps = { latestNews };
    return { props };
  } catch (error) {
    console.log(error);
    return { props: { latestNews: [] } };
  }
}
