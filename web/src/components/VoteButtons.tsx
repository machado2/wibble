import styles from "./NewsListItemUI.module.css";
import { downvote, unvote, upvote } from "./clientvote";
import classNames from "classnames";
import { BiUpvote, BiDownvote } from "react-icons/bi";
import { useEffect, useState } from "react";
import { signIn, useSession } from "next-auth/react";

export const VoteButtons = (props: {
  contentId?: string;
  currentVote?: number;
  votes: number;
}) => {
  const [voteCount, setVoteCount] = useState(props.votes);
  const [currentVote, setCurrentVote] = useState(props.currentVote);
  const [error, setError] = useState("");
  const [saving, setSaving] = useState(false);
  const { data: session } = useSession();

  useEffect(() => {
    setVoteCount(props.votes);
    setCurrentVote(props.currentVote);
    setError("");
  }, [props.contentId, props.currentVote, props.votes]);

  if (!props.contentId) {
    return null;
  }

  const id = props.contentId!;

  const isLogged = session && session.user ? true : false;

  const handleVote = async (direction: 1 | -1) => {
    if (!isLogged) {
      await signIn();
      return;
    }

    const previousVote = currentVote ?? 0;
    const previousCount = voteCount;
    const removingVote = previousVote === direction;
    const adjustment = removingVote
      ? -direction
      : previousVote === -direction
      ? direction * 2
      : direction;
    const nextVote = removingVote ? 0 : direction;

    setVoteCount(previousCount + adjustment);
    setCurrentVote(nextVote);
    setSaving(true);
    setError("");
    try {
      const response = removingVote
        ? await unvote(id)
        : direction === 1
        ? await upvote(id)
        : await downvote(id);
      if (!response.ok) {
        if (response.status === 401) {
          await signIn();
        }
        throw new Error("Vote could not be saved.");
      }
    } catch (voteError) {
      console.error(voteError);
      setVoteCount(previousCount);
      setCurrentVote(previousVote);
      setError("Vote could not be saved.");
    } finally {
      setSaving(false);
    }
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
        disabled={saving}
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
        disabled={saving}
      >
        <BiDownvote
          className={classNames({ [styles.voted]: currentVote === -1 })}
        />
      </button>
      {error ? <span role="status">{error}</span> : null}
    </div>
  );
};
