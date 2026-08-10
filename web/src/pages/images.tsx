import { Card } from "antd";
import styles from "@/styles/newsList.module.css";
import Head from "next/head";
import SearchBox from "@/components/SearchBox";
import { SortSelection } from "@/components/SortSelection";
import ImageList from "@/components/ImageList";
import { NewsImageSelector } from "@/components/NewsImageSelector";
import { TimeSelection } from "@/components/TimeSelection";

const siteDescription = `Get the latest news with a touch of wobble from The Wibble,
your source for the unpredictable and unsteady world of current events.`;
const siteTitle = "The Wibble";

export default function Page() {

  const sortOptions = [
    {
      value: "recent",
      label: "New",
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
        <ImageList />
      </Card>
    </>
  );
}
