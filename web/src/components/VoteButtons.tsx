import styles from "./NewsListItemUI.module.css";
import { downvote, unvote, upvote } from "./clientvote";
import classNames from "classnames";
import { BiUpvote, BiDownvote } from "react-icons/bi";
import { useState } from "react";
import { signIn, useSession } from "next-auth/react";

export const VoteButtons = (props: {
  contentId?: string;
  currentVote?: number;
  votes: number;
}) => {
  const [voteCount, setVoteCount] = useState(props.votes);
  const [currentVote, setCurrentVote] = useState(props.currentVote);
  const { data: session } = useSession();

  if (!props.contentId) {
    return null;
  }

  const id = props.contentId!;

  const isLogged = session && session.user ? true : false;

  const handleVote = async (direction: 1 | -1) => {
    if (!isLogged) {
      await signIn();
    }

    setCurrentVote((prevVote) => {
      let adjustment: number = direction;
      if (prevVote == direction) {
        // User is undoing their vote
        setVoteCount((prevCount) => prevCount - adjustment);
        unvote(id);
        return 0;
      } else if (prevVote === -direction) {
        // User is changing their vote
        adjustment = direction * 2;
      }
      setVoteCount((prevCount) => prevCount + adjustment);
      direction === 1 ? upvote(id) : downvote(id);
      return direction;
    });
  };

  const handleUpvote = () => {
    handleVote(1);
  };
  const handleDownvote = () => {
    handleVote(-1);
  };

  return (
    <div className={styles.voteContainer}>
      <button
        className={styles.upvoteButton}
        onClick={handleUpvote}
        aria-label="Upvote"
      >
        <BiUpvote
          className={classNames({ [styles.voted]: currentVote === 1 })}
        />
      </button>
      <span className={styles.voteCount}>{voteCount}</span>
      <button
        className={styles.downvoteButton}
        onClick={handleDownvote}
        aria-label="Downvote"
      >
        <BiDownvote
          className={classNames({ [styles.voted]: currentVote === -1 })}
        />
      </button>
    </div>
  );
};
