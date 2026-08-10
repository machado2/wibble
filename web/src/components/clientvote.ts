import { dontWaitFor } from "@/core/dontWaitFor";

export const upvote = (contentId: string) => {
  dontWaitFor(
    fetch("/api/vote", {
      method: "POST",
      headers: {
        "Content-Type": "application/json",
      },
      body: JSON.stringify({ content_id: contentId }),
    })
  );
};

export const unvote = (contentId: string) => {
  dontWaitFor(
    fetch("/api/unvote", {
      method: "POST",
      headers: {
        "Content-Type": "application/json",
      },
      body: JSON.stringify({ content_id: contentId }),
    })
  );
};

export const downvote = (contentId: string) => {
  dontWaitFor(
    fetch("/api/vote", {
      method: "POST",
      headers: {
        "Content-Type": "application/json",
      },
      body: JSON.stringify({ content_id: contentId, downvote: true }),
    })
  );
};
