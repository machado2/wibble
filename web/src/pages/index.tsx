import { Card } from "antd";
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
import SearchBox from "@/components/SearchBox";
import { SortSelection } from "@/components/SortSelection";
import { NewsImageSelector } from "@/components/NewsImageSelector";
import { TimeSelection } from "@/components/TimeSelection";
import { globalCopyForLanguage } from "@/components/GlobalLanguage";
import { resolveRequestLanguage } from "@/core/globalLanguageRequest";

const siteTitle = "The Wibble";
const PAGE_SIZE = 20;

type HomeProps = {
  latestNews: NewsListItem[];
  languageCode: string | null;
};

export default function Home(props: HomeProps) {
  const siteDescription = globalCopyForLanguage(props.languageCode).siteDescription;
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
        <ArticleList latestNews={props.latestNews} />
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
    const languageResolution = resolveRequestLanguage(
      typeof context.query.lang === "string" ? context.query.lang : undefined,
      context.req.headers.cookie,
      context.req.headers["accept-language"]
    );
    const languageCode = languageResolution.language;
    if (languageResolution.redirect && languageCode) {
      const parameters = new URLSearchParams();
      for (const [name, value] of Object.entries(context.query)) {
        if (name === "afterId" || name === "lang" || value === undefined) continue;
        parameters.set(name, Array.isArray(value) ? value[0] : value);
      }
      parameters.set("lang", languageCode);
      return {
        redirect: { destination: `/?${parameters.toString()}`, permanent: false },
      };
    }
    const latestNews: NewsListItem[] = await repo.getNextPage(
      afterId,
      sort,
      days,
      PAGE_SIZE,
      searchTerm,
      email ?? undefined,
      model,
      languageCode ?? undefined
    );

    const props: HomeProps = { latestNews, languageCode };
    return { props };
  } catch (error) {
    console.log(error);
    return { props: { latestNews: [], languageCode: null } };
  }
}
