import { NewsListItem } from "@/core/NewsListItem";
import { BoxedImage } from "./BoxedImage";
import styles from "./NewsListItemUI.module.css";
import Link from "next/link";
import { VoteButtons } from "./VoteButtons";
import { getHumanReadableDate } from "@/core/getHumanReadableDate";
import { useGlobalLanguage } from "./GlobalLanguage";

export const NewsListItemUI = (props: { item: NewsListItem }) => {
  const { item } = props;
  const { language } = useGlobalLanguage();
  const humanReadableDate = getHumanReadableDate(item.created_at);

  return (
    <>
      {item?.imagePrompt && (
        <BoxedImage
          prompt={item.imagePrompt}
          alt={item.description || item.title}
          className={styles.thumbnail}
        />
      )}
      <Link
        href={{
          pathname: "/content/[slug]",
          query: language ? { slug: item.slug, lang: language } : { slug: item.slug },
        }}
        target="_blank"
        className={styles.titleLink}
      >
        {item.title}
      </Link>
      <p className={styles.newsDate}>{humanReadableDate}</p>
      <p className={styles.description}>{item.description}</p>
      <div className={styles.bottom}>
        <VoteButtons
          contentId={item.id}
          votes={item.votes}
          currentVote={item.currentVote}
        />
      </div>
    </>
  );
};
