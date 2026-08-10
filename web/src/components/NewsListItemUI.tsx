import { NewsListItem } from "@/core/NewsListItem";
import { BoxedImage } from "./BoxedImage";
import styles from "./NewsListItemUI.module.css";
import Link from "next/link";
import { VoteButtons } from "./VoteButtons";
import { getHumanReadableDate } from "@/core/getHumanReadableDate";

export const NewsListItemUI = (props: { item: NewsListItem }) => {
  const { item } = props;
  const humanReadableDate = getHumanReadableDate(item.created_at);

  return (
    <>
      {item?.imagePrompt && (
        <BoxedImage
          prompt={item.imagePrompt}
          alt={item.imagePrompt}
          className={styles.thumbnail}
        />
      )}
      <Link href={`/content/${item.slug}`} target="_blank" className={styles.titleLink}>
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
